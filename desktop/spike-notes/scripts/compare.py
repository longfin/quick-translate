"""Side-by-side measurement of the installed Swift app and the Tauri spike, same session, same method.

Only one app runs at a time (both react to ⌘C ⌘C). Trigger = two real pasteboard writes 120 ms apart
(the user-facing ⌘C ⌘C path in both apps); t0 = when the second `pbcopy` returns.
Run with: python3 -I compare.py <path-to-waitwin-binary>
"""
import datetime, json, os, re, statistics, subprocess, sys, time

WAITWIN = sys.argv[1]
HOME = os.path.expanduser("~")
SPIKE = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../src-tauri/target/release/bundle/macos/QuickTranslateSpike.app"))
APPS = {
    "swift": dict(path="/Applications/QuickTranslate.app", exe="/Applications/QuickTranslate.app/Contents/MacOS/QuickTranslate",
                  log=f"{HOME}/Library/Logs/QuickTranslate.log", owner="QuickTranslate",
                  ready="pasteboard watcher started", trig="pasteboard changed twice", first=None, done="translate finished"),
    "tauri": dict(path=SPIKE, exe=SPIKE + "/Contents/MacOS/quicktranslate-spike",
                  log=f"{HOME}/Library/Logs/QuickTranslateSpike.log", owner="QuickTranslateSpike",
                  ready="TRAY_READY", trig="TRIGGER", first="FIRST_DELTA", done="RESULT_DONE"),
}
TEXTS = [
    "The new release improves startup time and reduces memory usage on older machines. Please update at your earliest convenience and report any regressions you notice.",
    "Our team will be offline on Friday for the company retreat. Urgent issues should be sent to the on-call engineer listed in the handbook.",
    "This library parses configuration files lazily, so errors only surface when a value is first accessed. Consider validating the whole file at startup instead.",
    "The museum extended its opening hours for the summer exhibition. Tickets can be booked online, and children under twelve enter for free.",
    "We measured the latency of each request and found that most of the time was spent waiting for the database. Adding an index reduced the median from 80 ms to 12 ms.",
]
WARMUP = "Warm-up sentence for the measurement run. It is not counted."
SWIFT_TS = re.compile(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3}) (.*)$")
TAURI_TS = re.compile(r"^\[(\d+)\] \S+(?: trig\+\S+)? (.*)$")


def sh(*a, **kw):
    return subprocess.run(a, capture_output=True, text=True, **kw).stdout


def pids(pattern):
    return [int(p) for p in sh("pgrep", "-f", pattern).split()]


def log_lines(app, offset):
    """(epoch_ms, message) for every line appended after `offset`."""
    with open(app["log"], "rb") as f:
        f.seek(offset)
        data = f.read().decode("utf-8", "replace")
    out = []
    for line in data.splitlines():
        m = SWIFT_TS.match(line)
        if m:
            ts = datetime.datetime.strptime(m.group(1), "%Y-%m-%d %H:%M:%S.%f").timestamp() * 1000
            out.append((ts, m.group(2)))
            continue
        m = TAURI_TS.match(line)
        if m:
            out.append((int(m.group(1)), m.group(2)))
    return out


def wait_for(app, offset, marker, timeout=25):
    end = time.time() + timeout
    while time.time() < end:
        for ts, msg in log_lines(app, offset):
            if marker in msg:
                return ts
        time.sleep(0.02)
    return None


def kill_all():
    for app in APPS.values():
        for p in pids(app["exe"]):
            subprocess.run(["kill", str(p)])
    for _ in range(100):
        if not any(pids(a["exe"]) for a in APPS.values()):
            break
        time.sleep(0.05)
    time.sleep(0.5)


def size(path):
    return os.path.getsize(path) if os.path.exists(path) else 0


def launch(name):
    app = APPS[name]
    off = size(app["log"])
    wk_before = set(pids("com.apple.WebKit"))
    t0 = time.time() * 1000
    subprocess.run(["open", "-n", app["path"]] if name == "tauri" else ["open", app["path"]])
    ready = wait_for(app, off, app["ready"], 15)
    return (ready - t0) if ready else None, wk_before


def footprint_mb(pid):
    m = re.search(r"Footprint: ([\d.]+) (KB|MB)", sh("footprint", "-p", str(pid)))
    if not m:
        return None
    v = float(m.group(1))
    return v / 1024 if m.group(2) == "KB" else v


def rss_mb(pid):
    out = sh("ps", "-o", "rss=", "-p", str(pid)).strip()
    return int(out) / 1024 if out else None


def memory(name, wk_before):
    app = APPS[name]
    pid = pids(app["exe"])[0]
    helpers = sorted(set(pids("com.apple.WebKit")) - wk_before) if name == "tauri" else []
    workers = [int(p) for p in sh("pgrep", "-P", str(pid)).split()]
    r = dict(app_rss=rss_mb(pid), app_fp=footprint_mb(pid),
             helpers_rss=sum(rss_mb(h) or 0 for h in helpers), helpers_fp=sum(footprint_mb(h) or 0 for h in helpers),
             n_helpers=len(helpers),
             worker_rss=sum(rss_mb(w) or 0 for w in workers), worker_fp=sum(footprint_mb(w) or 0 for w in workers))
    return r


def panel_on_screen(owner):
    p = subprocess.run([WAITWIN, owner, "panel", "0.05"], capture_output=True, text=True)
    return p.returncode == 0


def trigger(name, text):
    app = APPS[name]
    hidden_before = not panel_on_screen(app["owner"])
    off = size(app["log"])
    ww = subprocess.Popen([WAITWIN, app["owner"], "panel", "20"], stdout=subprocess.PIPE, text=True)
    time.sleep(0.15)
    subprocess.run(["pbcopy"], input=text, text=True)
    time.sleep(0.12)
    subprocess.run(["pbcopy"], input=text, text=True)
    t0 = time.time() * 1000
    done = wait_for(app, off, app["done"], 25)
    lines = log_lines(app, off)
    trig = next((ts for ts, m in lines if app["trig"] in m), None)
    first = next((ts for ts, m in lines if app["first"] and app["first"] in m), None)
    out, _ = ww.communicate(timeout=25)
    vis = int(out.strip()) if out.strip().isdigit() else None
    rel = lambda ts: round(ts - t0) if ts else None
    return dict(app=name, hidden_before=hidden_before, detect=rel(trig),
                visible=rel(vis) if hidden_before else None, first=rel(first), done=rel(done))


def main():
    results = {"cold": [], "fresh": [], "warm": [], "mem": []}
    # Phase 1: alternate cold launches. Each: cold start, idle memory, first translation on the fresh worker.
    for r in range(5):
        for name in ("swift", "tauri"):
            kill_all()
            cold, wk = launch(name)
            time.sleep(10)
            mem = memory(name, wk)
            res = trigger(name, TEXTS[r])
            results["cold"].append(dict(app=name, ms=round(cold) if cold else None))
            results["mem"].append(dict(app=name, **mem))
            results["fresh"].append(res)
            print(name, "cold", cold, "mem", mem, "trigger", res, flush=True)
            time.sleep(2)
    # Phase 2: warm worker, one launch per app, warm-up turn, then the 5 texts.
    for name in ("swift", "tauri"):
        kill_all()
        launch(name)
        time.sleep(10)
        trigger(name, WARMUP)
        time.sleep(2)
        for t in TEXTS:
            res = trigger(name, t)
            results["warm"].append(res)
            print(name, "warm", res, flush=True)
            time.sleep(2)
    kill_all()
    print("JSON " + json.dumps(results))


main()
