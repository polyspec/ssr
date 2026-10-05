"""The guard of the full suite (``make check``) and of the rerun of its failed targets
(``make rerun-failed``).

Before any step runs, the guard decides and prints the decision with its reasons. It refuses
both entries while an item of ``docs/checklist.md``, sub-items included, is in progress (``[~]``),
naming each such item, while tracked files have uncommitted changes, while files that are neither
tracked nor ignored exist, which a step would read although the tree does not hold them, and while
the pre-push hook is not installed (``tools/push_gate.py hooks-check``), and while a tool differs from
``tools/tool-versions.json`` (``tools/tool_versions.py``). ``check`` is refused when
the record names a full run of the current tree (``git rev-parse HEAD^{tree}``): the full suite
runs once per tree. ``rerun-failed`` is refused unless the record is of the current tree and names
targets that did not pass; it runs only those targets.

The record ``var/full-run.json`` (ignored by Git) holds the last full run of the checkout: the
tree, the commit, the result, the targets that did not pass, every setup step and target with its
status, exit status and times, and each rerun. The guard writes it before the first step and after
every step starts and ends, so a run that is killed stays recorded with the result
``incomplete``. Setup steps install the inputs that the targets read; they run before the targets
of both entries and are not targets. A failed setup step stops the run. A failed target does not
stop the run, so one run reports every target. Steps are make targets of the checkout; each step
reports its start, result and elapsed time, and none has a time limit.

    python3 -m tools.full_run check [--root <checkout>] --setup <target>... --targets <target>...
    python3 -m tools.full_run rerun-failed [--root <checkout>] --setup <target>...
"""

from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time

from tools import push_gate, tool_versions

ROOT = Path(__file__).resolve().parents[1]
ENTRIES = {"check": "make check", "rerun-failed": "make rerun-failed"}
ITEM = re.compile(r"^\s*- \[(.)\] (\S+)\s+(.*)$", re.MULTILINE)
SENTENCE = re.compile(r"(.+?\.)(?:\s|$)")


class Decision:
    def __init__(self, allowed, reasons, targets):
        self.allowed = allowed
        self.reasons = reasons
        self.targets = targets


def active_items(checklist):
    """The ``(id, title)`` of every item in progress; the title is the first sentence."""
    items = []
    for state, item, text in ITEM.findall(checklist):
        if state == "~":
            sentence = SENTENCE.match(text)
            items.append((item, sentence.group(1) if sentence else text.strip()))
    return items


def describe(record):
    failed = ", ".join(record["failed"]) or "none"
    return (f"the full run of tree {record['tree']} started {record['started']}, ended "
            f"{record['ended'] or 'never (killed or still running)'}, result {record['result']}, "
            f"targets that did not pass: {failed}")


def unfinished(record):
    return [target["name"] for target in record["targets"] if target["status"] != "passed"]


def decide(mode, items, changes, untracked, hooks, tools, tree, record, targets):
    """Decide whether ``mode`` may run; ``hooks`` are the reasons why the pre-push hook does not
    run, and ``targets`` are the targets of a full run."""
    reasons = []
    if items:
        reasons.append("checklist items are in progress (docs/checklist.md):")
        reasons += [f"  {item} {title}" for item, title in items]
    if changes:
        reasons.append("uncommitted changes of tracked files:")
        reasons += [f"  {change}" for change in changes]
    if untracked:
        reasons.append("untracked files, which the run would read but the tree does not hold:")
        reasons += [f"  {path}" for path in untracked]
    if tools:
        reasons.append("tools differ from tools/tool-versions.json:")
        reasons += [f"  {error}" for error in tools]
    if hooks:
        reasons.append("the pre-push hook is not installed:")
        reasons += [f"  {reason}" for reason in hooks]
    if mode == "check":
        if record is not None and record["tree"] == tree:
            reasons.append(f"{describe(record)}; the full suite runs once per tree"
                           + ("; rerun the targets that did not pass with make rerun-failed"
                              if record["failed"] else ""))
        selected = targets
    elif record is None:
        reasons.append("no full run is recorded (var/full-run.json); run make check")
        selected = []
    elif record["tree"] != tree:
        reasons.append(f"the recorded full run is of tree {record['tree']}, not of the current tree "
                       f"{tree}; rerun-failed reruns targets of the current tree only")
        selected = []
    else:
        selected = unfinished(record)
        if not selected:
            reasons.append(f"{describe(record)}; every target passed, nothing to rerun")
    return Decision(not reasons, reasons, selected)


def git(root, *arguments):
    result = subprocess.run(["git", *arguments], cwd=root, capture_output=True, text=True, check=False)
    if result.returncode:
        raise OSError(f"git {' '.join(arguments)} exited with {result.returncode}: {result.stderr.strip()}")
    return result.stdout


def now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3] + "Z"


def write(path, record):
    """Replace the record in one rename, so a reader never sees a partial record."""
    path.parent.mkdir(parents=True, exist_ok=True)
    written = path.with_name(f"{path.name}.{os.getpid()}")
    written.write_text(json.dumps(record, indent=2) + "\n")
    os.replace(written, path)


def read(path):
    if not path.exists():
        return None
    record = json.loads(path.read_text())
    for field in ("tree", "started", "result", "failed", "targets"):
        if field not in record:
            raise ValueError(f"{path} has no {field}; it is not a full run record")
    return record


class Interrupted(Exception):
    def __init__(self, number):
        super().__init__(f"signal {number}")
        self.number = number


def run_step(root, step, record, path):
    """Run one make target, recording its start and result. SIGINT, SIGTERM and SIGHUP go to the
    step; the run then stops with the step recorded as interrupted."""
    print(f"RUN {step['name']}", flush=True)
    step.update(status="running", exit=None, started=now(), ended=None)
    write(path, record)
    started = time.monotonic()
    child = subprocess.Popen(["make", "--no-print-directory", step["name"]], cwd=root)
    received = []

    def forward(number, _frame):
        received.append(number)
        child.send_signal(number)

    previous = {number: signal.signal(number, forward)
                for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)}
    try:
        status = child.wait()
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)
    elapsed = time.monotonic() - started
    step.update(status="interrupted" if received else "passed" if status == 0 else "failed",
                exit=status, ended=now())
    write(path, record)
    print(f"{'PASS' if status == 0 and not received else 'FAIL'} {step['name']} exit {status} "
          f"{elapsed:.1f}s", flush=True)
    if received:
        raise Interrupted(received[0])
    return status == 0


def finish(record):
    record["failed"] = unfinished(record)
    record["result"] = "failed" if record["failed"] else "passed"


def run(mode, setup, targets, root=ROOT):
    root = Path(root)
    path = root / "var/full-run.json"
    entry = ENTRIES[mode]
    tree = git(root, "rev-parse", "HEAD^{tree}").strip()
    changes = git(root, "status", "--porcelain", "--untracked-files=no").splitlines()
    untracked = [path for path in git(root, "ls-files", "-z", "--others", "--exclude-standard").split("\0") if path]
    items = active_items((root / "docs/checklist.md").read_text(encoding="utf-8"))
    record = read(path)
    decision = decide(mode, items, changes, untracked, push_gate.hooks_check(root), tool_versions.check(),
                      tree, record, targets)
    if not decision.allowed:
        print(f"full-run: refused {entry}:", flush=True)
        for reason in decision.reasons:
            print(f"full-run: {reason}", flush=True)
        return 2
    started = now()
    steps = [{"name": name, "status": "not run", "exit": None, "started": None, "ended": None}
             for name in setup]
    if mode == "check":
        print(f"full-run: allowed {entry}: no checklist item is in progress, the tracked files are "
              f"committed, no untracked file exists and no full run of tree {tree} is recorded; "
              f"running {len(targets)} targets: {' '.join(targets)}", flush=True)
        record = {
            "entry": entry, "tree": tree, "commit": git(root, "rev-parse", "HEAD").strip(),
            "started": started, "ended": None, "result": "incomplete", "failed": list(targets),
            "setup": steps,
            "targets": [{"name": name, "status": "not run", "exit": None, "started": None, "ended": None}
                        for name in targets],
            "reruns": [],
        }
        run_record = record
    else:
        print(f"full-run: allowed {entry}: the full run of tree {tree} recorded "
              f"{len(decision.targets)} targets that did not pass; rerunning only: "
              f"{' '.join(decision.targets)}", flush=True)
        run_record = {"started": started, "ended": None, "result": "incomplete",
                      "targets": decision.targets, "setup": steps}
        record["reruns"].append(run_record)
        record["result"] = "incomplete"
        record["ended"] = None
    write(path, record)
    selected = [target for target in record["targets"] if target["name"] in decision.targets]
    try:
        for step in steps:
            if not run_step(root, step, record, path):
                print(f"full-run: setup step {step['name']} failed; no target runs", flush=True)
                break
        else:
            for step in selected:
                run_step(root, step, record, path)
    except Interrupted as interrupted:
        print(f"full-run: interrupted by {interrupted}; {entry} is recorded as incomplete in {path}",
              flush=True)
        return 128 + interrupted.number
    finish(record)
    record["ended"] = run_record["ended"] = now()
    run_record["result"] = record["result"]
    write(path, record)
    print(f"full-run: {entry} result {record['result']}; targets that did not pass: "
          f"{', '.join(record['failed']) or 'none'}; record {path}", flush=True)
    if record["failed"]:
        print("full-run: run make rerun-failed to rerun only those targets", flush=True)
    return 1 if record["failed"] else 0


def main(argv):
    if argv and argv[0] in ENTRIES:
        mode, rest = argv[0], argv[1:]
        root = ROOT
        if rest[:1] == ["--root"] and len(rest) > 1:
            root, rest = Path(rest[1]), rest[2:]
        if rest[:1] == ["--setup"]:
            rest = rest[1:]
            separator = rest.index("--targets") if "--targets" in rest else len(rest)
            setup, targets = rest[:separator], rest[separator + 1:]
            if setup and (mode == "check") == bool(targets) and (mode == "check") == ("--targets" in rest):
                return run(mode, setup, targets, root)
    print("usage: python3 -m tools.full_run check [--root <checkout>] --setup <target>... --targets <target>...\n"
          "       python3 -m tools.full_run rerun-failed [--root <checkout>] --setup <target>...",
          file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
