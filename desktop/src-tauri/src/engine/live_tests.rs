//! End-to-end checks against the real, logged-in CLIs. Ignored by default (they cost tokens and
//! need network); run with:
//!
//! ```sh
//! cargo test live_ -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc;

use super::claude_worker::ClaudeWorker;
use super::settings::{Backend, Settings};
use super::translation_engine;
use super::{TranslationEvent, TranslationJob};

const TEXT: &str = "Hello, world";

struct Run {
    rx: mpsc::UnboundedReceiver<TranslationEvent>,
    job: Option<Box<dyn TranslationJob>>,
    started: Instant,
}

fn start(text: &str, settings: &Settings) -> Run {
    let (tx, rx) = mpsc::unbounded_channel();
    let sink = Arc::new(move |e: TranslationEvent| {
        let _ = tx.send(e);
    });
    let started = Instant::now();
    let job =
        translation_engine::translate(text, Some("English"), "Korean", "English", settings, sink);
    Run { rx, job, started }
}

/// Prints every event with its latency; returns the final text.
async fn finish(label: &str, mut run: Run) -> String {
    let mut acc = String::new();
    let mut first = None;
    loop {
        let e = tokio::time::timeout(Duration::from_secs(120), run.rx.recv())
            .await
            .unwrap_or_else(|_| panic!("{label}: timed out"))
            .unwrap_or_else(|| panic!("{label}: sink dropped without a terminal event"));
        let ms = run.started.elapsed().as_millis();
        match e {
            TranslationEvent::Delta(t) => {
                first.get_or_insert(ms);
                println!("[{label}] +{ms:>5} ms delta    {t:?}");
                acc.push_str(&t);
            }
            TranslationEvent::Finished(full) => {
                println!("[{label}] +{ms:>5} ms finished {full:?} (first delta at {first:?} ms)");
                return if full.is_empty() { acc } else { full };
            }
            TranslationEvent::Failed(msg) => panic!("[{label}] +{ms} ms failed: {msg}"),
        }
    }
}

fn claude_settings(warm: bool) -> Settings {
    Settings {
        backend: Backend::Claude,
        claude_model: "haiku".into(),
        keep_claude_warm: warm,
        ..Settings::default()
    }
}

fn assert_korean(s: &str) {
    assert!(
        s.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)),
        "not Korean: {s:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn live_claude_warm_worker() {
    let settings = claude_settings(true);
    let t = Instant::now();
    translation_engine::prewarm(&settings);
    // Like the app: prewarm at launch, the first request comes later.
    tokio::time::sleep(Duration::from_secs(3)).await;
    println!("prewarmed for {} ms", t.elapsed().as_millis());

    let out = finish("warm#1", start(TEXT, &settings)).await;
    assert_korean(&out);

    // Immediately again: exercises `/clear` and its own `result` line between turns.
    let out = finish("warm#2", start("Good night, see you tomorrow.", &settings)).await;
    assert_korean(&out);

    // Cancel mid-turn: no further events, then the respawned worker serves the next request.
    let mut run = start(
        "Please translate this rather long sentence about the weather in Seoul today.",
        &settings,
    );
    let first = tokio::time::timeout(Duration::from_secs(60), run.rx.recv())
        .await
        .unwrap()
        .unwrap();
    println!(
        "[cancel] +{} ms first event {first:?} -> cancel",
        run.started.elapsed().as_millis()
    );
    run.job.take().unwrap().cancel();
    tokio::time::sleep(Duration::from_millis(800)).await;
    let mut late = vec![];
    while let Ok(e) = run.rx.try_recv() {
        late.push(e);
    }
    // Deltas already queued before the kill may still be there; never a terminal event.
    assert!(
        !late
            .iter()
            .any(|e| !matches!(e, TranslationEvent::Delta(_))),
        "terminal event after cancel: {late:?}"
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    let out = finish("after-cancel", start(TEXT, &settings)).await;
    assert_korean(&out);

    ClaudeWorker::shared().shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn live_claude_one_shot() {
    let out = finish("one-shot", start(TEXT, &claude_settings(false))).await;
    assert_korean(&out);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn live_codex() {
    let settings = Settings {
        backend: Backend::Codex,
        ..Settings::default()
    };
    let out = finish("codex", start(TEXT, &settings)).await;
    assert_korean(&out);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn live_cli_versions() {
    for b in [Backend::Claude, Backend::Codex] {
        let info = super::commands::check_cli(b, "").await.unwrap();
        println!("{} at {} -> {}", info.name, info.path, info.version);
    }
}
