"""Compare complete check reports across frozen implementations and allowances."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
BASELINE_SHA256 = "1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0"
CURRENT_SHA256 = "93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9"
BASELINE_REVISION = "32b5d157fea3512cd9f624bdff5a3f3a4b3d2e2b"
CURRENT_REVISION = "57140e145eaef59b3de6f8b84eca6852a8526ea8"
CORPUS_REPORT_SHA256 = "d802ab628752d517071ddd2e16afd7aa1b6cae2b36c6c86a45da1ebc33da2cc0"
EXPLICIT_STEPS = "10000000"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return path.resolve().relative_to(ROOT).as_posix()


def invoke(cli, database, explicit):
    arguments = [
        relative(cli), "--format", "tsv", "inspect", relative(database),
        "--table", "Indexed", "--check",
    ]
    if explicit:
        arguments.extend(["--native-steps", EXPLICIT_STEPS])
    result = subprocess.run(
        arguments, cwd=ROOT, capture_output=True, text=True,
        encoding="utf-8", check=False, timeout=60,
    )
    return {
        "command": arguments,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }


def successful(record):
    return record["exit_code"] == 0 and record["stderr"] == ""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--corpus-report", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.report.exists():
        raise FileExistsError("Refusing to overwrite report")
    for path, expected in [
        (args.baseline, BASELINE_SHA256),
        (args.current, CURRENT_SHA256),
        (args.corpus_report, CORPUS_REPORT_SHA256),
    ]:
        if digest(path) != expected:
            raise ValueError("Frozen implementation or registered corpus differs")

    corpus = json.loads(args.corpus_report.read_text())
    inputs = {
        Path(pin["path"]).name: pin
        for pin in corpus["pins"] if pin["path"].endswith(".accdb")
    }
    if len(inputs) != 16 or len(corpus["observations"]) != 16:
        raise ValueError("Expected sixteen registered original states")
    pinned = [
        args.baseline, args.current, args.corpus_report, Path(__file__),
        *(ROOT / item["path"] for item in inputs.values()),
    ]
    before = {relative(path): digest(path) for path in pinned}
    for item in inputs.values():
        if digest(ROOT / item["path"]) != item["sha256"]:
            raise ValueError("Original database differs")

    controls = []
    for observation in corpus["observations"]:
        profile = observation["profile"]
        stage = observation["stage"]
        name = f"Delete-{profile}-{stage}.accdb"
        database = ROOT / inputs[name]["path"]
        old_explicit = invoke(args.baseline, database, True)
        new_explicit = invoke(args.current, database, True)
        new_default = invoke(args.current, database, False)
        old_default = invoke(args.baseline, database, False)
        expected_rows = len(observation["tables"]["Indexed"]["rows"])
        complete = successful(old_explicit)
        complete = complete and f"Rows checked\t{expected_rows}\n" in old_explicit["stdout"]
        complete = complete and "GroupIgnoreNulls\tNot checked: unsupported profile" in old_explicit["stdout"]
        for label, record in [
            ("baseline-explicit", old_explicit),
            ("current-explicit", new_explicit),
            ("current-default", new_default),
            ("baseline-default", old_default),
        ]:
            if label == "baseline-default" and stage != "delete-all":
                matched = record["exit_code"] != 0 and record["stdout"] == ""
                matched = matched and "Steps resource limit exceeded" in record["stderr"]
            else:
                matched = complete and successful(record)
                matched = matched and record["stdout"] == old_explicit["stdout"]
            controls.append({
                "profile": profile, "stage": stage, "case": label,
                **record, "matched": matched,
            })

    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original native full-check report equivalence and default-work completion; no timing or native Access qualification",
        "baseline_revision": BASELINE_REVISION,
        "current_revision": CURRENT_REVISION,
        "explicit_steps": int(EXPLICIT_STEPS),
        "default_steps": 1_000_000,
        "commands": controls,
        "command_count": len(controls),
        "disagreements": sum(not item["matched"] for item in controls),
        "baseline_expected_default_work_refusals": 12,
        "current_default_complete_reports": sum(
            item["case"] == "current-default" and successful(item) for item in controls
        ),
        "immutable_before": before,
        "immutable_after": after,
        "pins_unchanged": before == after,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({key: report[key] for key in [
        "command_count", "disagreements", "current_default_complete_reports", "pins_unchanged",
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
