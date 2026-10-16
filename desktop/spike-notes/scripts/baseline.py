"""Baseline: same resident claude CLI + same args, driven directly (no Tauri), same 5 texts."""
import json, os, subprocess, tempfile, time, statistics

SYS = """You are a professional translation engine.
Each user message starts with an instruction line ending in a colon, followed by the text to translate.
Preserve the original meaning, tone, formatting, line breaks, markdown and code blocks. Do not translate code identifiers, URLs or proper nouns that are normally left as-is.
Output ONLY the translated text. No explanations, no notes, no quotes, no preamble."""
INSTR = "Translate into Korean, or into English if the text is already mostly Korean:"
TEXTS = [
    "The new release improves startup time and reduces memory usage on older machines. Please update at your earliest convenience and report any regressions you notice.",
    "Our team will be offline on Friday for the company retreat. Urgent issues should be sent to the on-call engineer listed in the handbook.",
    "This library parses configuration files lazily, so errors only surface when a value is first accessed. Consider validating the whole file at startup instead.",
    "The museum extended its opening hours for the summer exhibition. Tickets can be booked online, and children under twelve enter for free.",
    "We measured the latency of each request and found that most of the time was spent waiting for the database. Adding an index reduced the median from 80 ms to 12 ms.",
]
env = dict(os.environ, NO_COLOR="1", TERM="dumb", MAX_THINKING_TOKENS="0")
p = subprocess.Popen(
    [os.path.expanduser("~/.local/bin/claude"), "-p", "--input-format", "stream-json", "--output-format", "stream-json",
     "--verbose", "--include-partial-messages", "--tools", "", "--strict-mcp-config", "--setting-sources", "",
     "--no-session-persistence", "--system-prompt", SYS, "--model", "haiku"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=env, cwd=tempfile.gettempdir(), text=True, bufsize=1)
def send(c):
    p.stdin.write(json.dumps({"type": "user", "message": {"role": "user", "content": c}}) + "\n"); p.stdin.flush()
time.sleep(3)  # let it warm like the prewarmed worker
firsts, dones = [], []
for t in TEXTS:
    t0 = time.perf_counter(); first = None
    send(INSTR + "\n\n" + t)
    for line in p.stdout:
        o = json.loads(line)
        if o.get("type") == "stream_event" and first is None:
            ev = o["event"]
            if ev.get("type") == "content_block_delta" and ev["delta"].get("type") == "text_delta":
                first = time.perf_counter() - t0
        if o.get("type") == "result":
            d = time.perf_counter() - t0
            print(f"first={first*1000:.0f}ms done={d*1000:.0f}ms  {o.get('result','')[:40]!r}")
            firsts.append(first); dones.append(d)
            break
    tc = time.perf_counter(); send("/clear")
    for line in p.stdout:
        o = json.loads(line)
        if o.get("type") == "result":
            print(f"   /clear result after {(time.perf_counter()-tc)*1000:.0f}ms: {o.get('result','')[:30]!r}"); break
    time.sleep(1.5)
print(f"median first={statistics.median(firsts)*1000:.0f}ms done={statistics.median(dones)*1000:.0f}ms")
p.stdin.close(); p.terminate()
