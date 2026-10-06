"""The guard of the full suite (``make check``) and of the rerun of its failed targets
(``make rerun-failed``).

Before any step runs, the guard decides and prints the decision with its reasons. It refuses
both entries while an item of ``docs/checklist.md``, sub-items included, is in progress (``[~]``),
naming each such item, while tracked files have uncommitted changes, while files that are neither
tracked nor ignored exist, which a step would read although the tree does not hold them, and while
the pre-push hook is not installed (``tools/push_gate.py hooks-check``), and while a tool differs from
``tools/tool-versions.json`` of the checkout (``tools/tool_versions.py``). ``check`` is refused when
the record names a full run of the current tree (``git rev-parse HEAD^{tree}``): the full suite
runs once per tree. ``rerun-failed`` is refused unless the record is of the current tree and names
targets that did not pass; it runs only those targets.

The record ``var/full-run.json`` (ignored by Git) holds the last full run of the checkout: the
tree, the commit, the result, the targets that did not pass, every setup step and target with its
status, exit status and times, and each rerun. The guard writes it before the first step and after
every step starts and ends, so a run that is killed stays recorded with the result
``incomplete``. Setup steps install the inputs that the targets read; they run before the targets
of both entries and are not targets. Every setup step runs, also after a failed one. ``--needs``
names the setup steps that a target reads (``<target>=<setup>,<setup>``); a target whose setup step
failed is recorded as ``skipped`` with that step, and every other target runs. A failed target does
not stop the run, so one run reports every target. Steps are make targets of the checkout; each step
reports its start, result and elapsed time, and none has a time limit.

    python3 -m tools.full_run check [--root <checkout>] --setup <target>... --targets <target>...
        [--needs <target>=<setup>,...]
    python3 -m tools.full_run rerun-failed [--root <checkout>] --setup <target>...
        [--needs <target>=<setup>,...]
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
    """Run one make target, recording its start and result. SIGINT, SIGTERM and SIGHUP go to every
    process of the step; the run then stops with the step recorded as interrupted."""
    print(f"RUN {step['name']}", flush=True)
    step.pop("reason", None)
    step.update(status="running", exit=None, started=now(), ended=None)
    write(path, record)
    started = time.monotonic()
    # The step runs in a process group of its own, and a signal goes to the whole group, so every
    # process that the step started, not only make, receives it.
    child = subprocess.Popen(["make", "--no-print-directory", step["name"]], cwd=root, start_new_session=True)
    received = []

    def forward(number, _frame):
        received.append(number)
        try:
            os.killpg(child.pid, number)
        except ProcessLookupError:
            pass

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


def run(mode, setup, targets, root=ROOT, needs=None):
    needs = needs or {}
    root = Path(root)
    path = root / "var/full-run.json"
    entry = ENTRIES[mode]
    tree = git(root, "rev-parse", "HEAD^{tree}").strip()
    changes = git(root, "status", "--porcelain", "--untracked-files=no").splitlines()
    untracked = [path for path in git(root, "ls-files", "-z", "--others", "--exclude-standard").split("\0") if path]
    items = active_items((root / "docs/checklist.md").read_text(encoding="utf-8"))
    record = read(path)
    decision = decide(mode, items, changes, untracked, push_gate.hooks_check(root),
                      tool_versions.check(tools=tool_versions.declared(root / "tools/tool-versions.json")),
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
        failed_setup = [step["name"] for step in steps if not run_step(root, step, record, path)]
        for step in selected:
            blocked = [name for name in failed_setup if name in needs.get(step["name"], ())]
            if blocked:
                step.update(status="skipped", exit=None, started=None, ended=None,
                            reason=f"setup step {', '.join(blocked)} failed")
                write(path, record)
                print(f"SKIP {step['name']}: it reads the output of the failed setup step "
                      f"{', '.join(blocked)}", flush=True)
                continue
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


USAGE = ("usage: python3 -m tools.full_run check [--root <checkout>] --setup <target>... --targets <target>... "
         "[--needs <target>=<setup>,...]\n"
         "       python3 -m tools.full_run rerun-failed [--root <checkout>] --setup <target>... "
         "[--needs <target>=<setup>,...]")


def parse(mode, rest):
    """The options of an entry, or None when they are invalid."""
    options = {"--root": [], "--setup": [], "--targets": [], "--needs": []}
    current = None
    for argument in rest:
        if argument in options:
            if options[argument] or argument == current:
                return None
            current = argument
        elif current is None:
            return None
        else:
            options[current].append(argument)
    if len(options["--root"]) > 1 or not options["--setup"] or (mode == "check") != bool(options["--targets"]):
        return None
    needs = {}
    for item in options["--needs"]:
        target, _, names = item.partition("=")
        steps = names.split(",") if names else []
        if (not target or target in needs or not steps
                or any(step not in options["--setup"] for step in steps)):
            return None
        needs[target] = steps
    root = Path(options["--root"][0]) if options["--root"] else ROOT
    return root, options["--setup"], options["--targets"], needs


def main(argv):
    if argv and argv[0] in ENTRIES:
        parsed = parse(argv[0], argv[1:])
        if parsed is not None:
            root, setup, targets, needs = parsed
            return run(argv[0], setup, targets, root, needs)
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
