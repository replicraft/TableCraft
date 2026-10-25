"""Model original large ACE files and explicitly spooled stored/MSZIP packages.

Without --cli this records only fixture/getter/model evidence. CLI acceptance
requires an explicitly frozen executable digest; no mutable build is invoked.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from build_large_deployment import digest, inspect

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ROWS = 1100
MEMO_UNITS = 32768
NATIVE_BYTES = "268435456"
SOURCE_PATHS = [
    "crates/backends/tablecraft-access/src/input/mod.rs",
    "crates/backends/tablecraft-access/src/input/deployment.rs",
    "crates/backends/tablecraft-access/src/input/deployment/layout.rs",
    "crates/backends/tablecraft-access/src/input/deployment/bounded.rs",
    "crates/backends/tablecraft-access/src/input/deployment/spool.rs",
    "crates/backends/tablecraft-access/src/input/deployment/hybrid.rs",
    "apps/tablecraft-cli/src/native.rs",
    "apps/tablecraft-cli/src/native/source.rs",
    "apps/tablecraft-cli/src/commands/inspect/args.rs",
    "apps/tablecraft-cli/src/commands/inspect/mod.rs",
    "apps/tablecraft-cli/src/commands/tables.rs",
    "apps/tablecraft-cli/src/commands/rows/mod.rs",
    "apps/tablecraft-cli/src/commands/rows/stream.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
]


def memo(identity):
    return bytes(32 + (identity * 17 + position * 29 + position // 97) % 95
                 for position in range(MEMO_UNITS))


def invoke(arguments):
    return subprocess.run([str(argument) for argument in arguments],
                          capture_output=True, text=True, timeout=120)


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def source_snapshots(revision):
    records = {}
    for name in SOURCE_PATHS:
        result = invoke(["git", "show", f"{revision}:{name}"])
        if result.returncode:
            raise RuntimeError(result.stderr)
        records[name] = hashlib.sha256(result.stdout.encode()).hexdigest()
    return records


def getter_model(jar, path):
    result = invoke([
        "java", "-Xmx256m", "-Dstdout.encoding=UTF-8", "-cp", jar,
        "docs/testing/LargeDeploymentFixture.java", "--inspect", path,
    ])
    if result.returncode:
        raise RuntimeError(result.stderr)
    all_memos = hashlib.sha256()
    selected = {}
    for identity in range(1, ROWS + 1):
        value = memo(identity)
        all_memos.update(value)
        if identity in (1, 550, ROWS):
            selected[identity] = hashlib.sha256(value).hexdigest()
    expected = ["Columns\tId\tScore\tLabel\tMemo"]
    expected.extend(
        f"Row\t{identity}\t{identity * 17 - 9}\trow-{identity:06d}\t"
        f"{MEMO_UNITS}\t{value_hash}" for identity, value_hash in selected.items()
    )
    identity_total = ROWS * (ROWS + 1) // 2
    score_total = identity_total * 17 - 9 * ROWS
    expected.append(
        f"Summary\t{ROWS}\t{identity_total}\t{score_total}\t"
        f"{ROWS * MEMO_UNITS}\t{all_memos.hexdigest()}"
    )
    if result.stdout.splitlines() != expected:
        raise RuntimeError("Read-only Java getters differ from independent Python model")
    return {
        "getter_stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
        "rows": ROWS, "identity_total": identity_total, "score_total": score_total,
        "memo_units": ROWS * MEMO_UNITS, "all_memos_sha256": all_memos.hexdigest(),
        "selected_memo_sha256": selected,
    }


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.observations = []

    def run(self, label, arguments, expected=None, refusal=False):
        result = invoke([self.cli, "--format", "tsv", *arguments])
        if refusal:
            if result.returncode == 0 or result.stdout or "resource" not in result.stderr.lower():
                raise RuntimeError(f"Expected empty resource refusal for {label}: {result}")
        elif result.returncode:
            raise RuntimeError(f"Unexpected CLI failure for {label}: {result}")
        if expected is not None and result.stdout != expected:
            raise RuntimeError(f"Exact output differs for {label}: {result.stdout!r}")
        self.observations.append({
            "case": label, "arguments": [str(argument) for argument in arguments],
            "exit_code": result.returncode, "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "refusal": refusal,
        })
        return result.stdout

    def source(self, path, model, packaged):
        label = path.name
        output = self.run(label + "/inspect", ["inspect", path, "--native-bytes", NATIVE_BYTES])
        expected_format = "Ace2016" if path.stem.startswith("V2016") else "Ace2019"
        if ["Format", expected_format] not in list(csv.reader(io.StringIO(output), delimiter="\t")):
            raise RuntimeError("Header format differs from authored profile")
        self.run(label + "/tables", ["tables", path, "--count", "--native-bytes", NATIVE_BYTES],
                 "Name\tKind\tRow Count\nLargeDeployment\tUser\t1100\n")
        for identity, value_hash in model["selected_memo_sha256"].items():
            output = self.run(label + f"/memo-{identity}", [
                "rows", path, "LargeDeployment", "--offset", identity - 1, "--limit", "1",
                "--native-bytes", NATIVE_BYTES,
            ])
            rows = list(csv.reader(io.StringIO(output), delimiter="\t"))
            if len(rows) != 2 or rows[0] != ["Id", "Score", "Label", "Memo"]:
                raise RuntimeError("Bounded native row shape differs")
            identity_text, score, label_text, memo_text = rows[1]
            value = json.loads(memo_text).encode("ascii")
            if (int(identity_text) != identity or int(score) != identity * 17 - 9
                    or json.loads(label_text) != f"row-{identity:06d}"
                    or len(value) != MEMO_UNITS or hashlib.sha256(value).hexdigest() != value_hash):
                raise RuntimeError("Selected native payload differs from independent model")
        flags = ["--database", path, "--native-bytes", NATIVE_BYTES,
                 "--native-steps", "10000000"]
        self.run(label + "/count", [
            "query", "run", "SELECT Count(*) AS N,Min(Id) AS Low,Max(Id) AS High,"
            "Sum(Score) AS S FROM LargeDeployment", *flags, "--stream",
        ], f"N\tLow\tHigh\tS\n{ROWS}\t1\t{ROWS}\t{model['score_total']}\n")
        for batch in (1, 128):
            output = self.run(label + f"/rows-{batch}", [
                "query", "run", "SELECT Id,Score,Label FROM LargeDeployment", *flags,
                "--stream", "--batch-rows", batch,
            ])
            rows = list(csv.reader(io.StringIO(output), delimiter="\t"))
            if rows[0] != ["Id", "Score", "Label"] or len(rows) != ROWS + 1:
                raise RuntimeError("Progressive native row count differs")
            for identity, row in enumerate(rows[1:], 1):
                if row != [str(identity), str(identity * 17 - 9), json.dumps(f"row-{identity:06d}")]:
                    raise RuntimeError("Progressive native rows differ from model")
        self.run(label + "/low-budget", ["inspect", path, "--native-bytes", "1"], refusal=True)
        if packaged:
            self.run(label + "/default-budget", ["inspect", path], refusal=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--cli-sha256")
    parser.add_argument("--revision")
    parser.add_argument("--provenance", type=Path)
    args = parser.parse_args()
    if args.report.exists() or digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Report must be fresh and reference reader must be reviewed")
    if args.cli and (not args.cli_sha256 or digest(args.cli) != args.cli_sha256):
        raise RuntimeError("CLI replay requires the exact frozen executable digest")
    revision = None
    snapshots = {}
    if args.cli:
        if not args.revision or not args.provenance:
            raise RuntimeError("CLI replay requires committed source revision and original provenance")
        resolved = invoke(["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"])
        if resolved.returncode:
            raise RuntimeError(resolved.stderr)
        revision = resolved.stdout.strip()
        snapshots = source_snapshots(revision)
    pinned = [args.jar, Path(__file__), ROOT / "docs/testing/LargeDeploymentFixture.java",
              ROOT / "docs/testing/build_large_deployment.py"]
    if args.cli:
        pinned.append(args.cli)
    if args.provenance:
        pinned.append(args.provenance)
    pinned.extend(sorted(args.directory.glob("*.accdb")))
    pinned.extend(sorted(args.directory.glob("*.accdc")))
    before = {relative(path): digest(path) for path in pinned}
    if sum(path.stat().st_size for path in args.directory.iterdir() if path.is_file()) > 500 * 1024 * 1024:
        raise RuntimeError("Original corpus exceeds agreed disk ceiling")
    replay = Replay(args.cli) if args.cli else None
    models = {}
    for profile in ("V2016", "V2019"):
        native = args.directory / f"{profile}.accdb"
        size = native.stat().st_size
        if not 64 * 1024 * 1024 < size < 100 * 1024 * 1024:
            raise RuntimeError("Original native image misses intended size range")
        model = getter_model(args.jar, native)
        with native.open("rb") as source:
            model["native_prefix64_hex"] = source.read(64).hex()
        models[profile] = model
        if replay:
            replay.source(native, model, False)
        for mode in ("stored", "mszip"):
            package = args.directory / f"{profile}-{mode}.accdc"
            model[mode] = inspect(package, size, before[relative(native)])
            if replay:
                replay.source(package, model, True)
    after = {relative(path): digest(path) for path in pinned}
    if before != after:
        raise RuntimeError("Immutable originals or tools changed")
    if revision and source_snapshots(revision) != snapshots:
        raise RuntimeError("Committed source snapshots changed")
    report = {
        "date": "2026-10-25", "stage": "cli-replay" if replay else "fixture-model-only",
        "scope": "Original ACE16-family getter/model/CAB engineering evidence, no native Access execution",
        "immutable_sha256_before": before, "immutable_sha256_after": after,
        "compiled_revision": revision,
        "scoped_revision_source_sha256": snapshots,
        "source_note": "Selected raw git-show snapshots, not a compilation manifest or live-source pins",
        "models": models, "observations": replay.observations if replay else [],
        "control_count": len(replay.observations) if replay else 0,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"stage": report["stage"], "control_count": report["control_count"]}))


if __name__ == "__main__":
    main()
