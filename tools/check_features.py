"""Execute every supported feature case declared in features.json."""

import json
import os
import pathlib
import re
import signal
import subprocess
import sys
import time


ROOT = pathlib.Path(__file__).resolve().parents[1]
CASE_TIMEOUT = 180


def unique_members(pairs):
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError(f"duplicate JSON member: {name}")
        result[name] = value
    return result


def load(path):
    document = json.loads(path.read_text(), object_pairs_hook=unique_members)
    if not isinstance(document, dict) or set(document) != {"features"}:
        raise ValueError("features.json must contain only a features array")
    features = document["features"]
    if not isinstance(features, list) or not features:
        raise ValueError("features must be a nonempty array")
    names = set()
    cases = []
    for feature in features:
        if not isinstance(feature, dict) or set(feature) != {"name", "supported", "test"}:
            raise ValueError("each feature requires name, supported and test")
        name = feature["name"]
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9-]*", name):
            raise ValueError(f"invalid feature name: {name!r}")
        if name in names:
            raise ValueError(f"duplicate feature name: {name}")
        names.add(name)
        if feature["supported"] is not True:
            raise ValueError(f"only verified supported features are declared: {name}")
        case = feature["test"]
        if not isinstance(case, dict) or set(case) != {"package", "binary", "case"}:
            raise ValueError(f"exact package, binary and case required: {name}")
        for field, pattern in (
            ("package", r"ssr-[a-z-]+"),
            ("binary", r"[a-z][a-z0-9_]*"),
            ("case", r"[a-z][a-z0-9_]*"),
        ):
            value = case[field]
            if not isinstance(value, str) or not re.fullmatch(pattern, value):
                raise ValueError(f"invalid {field} for {name}: {value!r}")
        cases.append((name, case))
    return cases


def output(value, destination):
    if value:
        if isinstance(value, bytes):
            destination.buffer.write(value)
        else:
            destination.write(value)
        destination.flush()


def run(command, *, timeout):
    process = subprocess.Popen(
        command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        raise subprocess.TimeoutExpired(command, timeout, output=stdout, stderr=stderr)
    except BaseException:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        raise
    return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def check(path, runner=run):
    failures = []
    cases = load(path)
    for name, case in cases:
        command = [
            "cargo", "nextest", "run", "--locked", "--no-tests", "fail",
            "-p", case["package"], "--test", case["binary"],
            "--", "--exact", case["case"],
        ]
        print(f"RUN feature {name}: {case['package']}::{case['binary']}::{case['case']}", flush=True)
        started = time.monotonic()
        try:
            result = runner(command, timeout=CASE_TIMEOUT)
            output(result.stdout, sys.stdout)
            output(result.stderr, sys.stderr)
            if result.returncode:
                failures.append(name)
                print(f"FAIL feature {name}: exit {result.returncode} ({time.monotonic()-started:.3f}s)", flush=True)
            else:
                print(f"PASS feature {name} ({time.monotonic()-started:.3f}s)", flush=True)
        except subprocess.TimeoutExpired as error:
            output(error.stdout, sys.stdout)
            output(error.stderr, sys.stderr)
            failures.append(name)
            print(f"TIMEOUT feature {name}: {CASE_TIMEOUT}s", flush=True)
    if failures:
        raise RuntimeError(f"feature cases failed: {', '.join(failures)}")


if __name__ == "__main__":
    try:
        check(ROOT / "features.json")
    except (OSError, ValueError, RuntimeError) as error:
        print(f"FAIL supported features: {error}", file=sys.stderr)
        sys.exit(1)
