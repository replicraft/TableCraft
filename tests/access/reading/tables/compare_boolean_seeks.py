"""Compare exact Boolean index selections with inert reference cursor getters."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
COMMAND_TIMEOUT_SECONDS = 60


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def recorded_command(args):
    return [relative(arg) if isinstance(arg, Path) else str(arg) for arg in args]


def command(args):
    return subprocess.run(
        [str(arg) for arg in args], capture_output=True, text=True,
        timeout=COMMAND_TIMEOUT_SECONDS,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--inputs", required=True, nargs=2, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Accepted report must be fresh")

    if digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Independent decoder artifact differs from reviewed Jackcess 5.0.4")

    pinned = [args.cli, args.jar, Path(__file__), *args.inputs]
    pinned.extend(Path(path) for path in [
        "docs/testing/BooleanIndexFixture.java",
        "docs/testing/IndexRowsOracle.java",
        "crates/engines/tablecraft-ace/src/table/indexes/selection.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/boolean.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/read.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/projection.rs",
        "crates/query/tablecraft-query/src/source/constraint.rs",
        "crates/query/tablecraft-query/src/execution/constraint.rs",
        "crates/backends/tablecraft-access/src/source/selection.rs",
    ])
    hashes = {relative(path): digest(path) for path in pinned}
    observations = []
    getters = {}
    compared_rows = 0
    for database in args.inputs:
        getter = command([
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/IndexRowsOracle.java", database,
            "BooleanIndexed", "BoolAsc", "ID", "Flag",
        ])
        if getter.returncode:
            raise RuntimeError(getter.stderr)
        cells = [line.split("\t") for line in getter.stdout.splitlines()]
        if len(cells) != 1000 or any(flag not in ("true", "false") for _, flag in cells):
            raise RuntimeError("Unexpected original Boolean getter domain")
        records = [(int(identity), flag == "true") for identity, flag in cells]
        getters[database.name] = {
            "rows": len(records),
            "true": sum(flag for _, flag in records),
            "false": sum(not flag for _, flag in records),
            "stdout_sha256": hashlib.sha256(getter.stdout.encode()).hexdigest(),
        }
        modes = [[], ["--auto-index"], ["--index", "BooleanIndexed", "BoolAsc"],
                 ["--index", "BooleanIndexed", "BoolDesc"]]
        for value in [False, True]:
            expected_ids = sorted(identity for identity, flag in records if flag == value)
            expected = "ID\n" + "".join(f"{identity}\n" for identity in expected_ids)
            sql = f"SELECT ID FROM BooleanIndexed WHERE Flag={'True' if value else 'False'}"
            for mode in modes:
                # A positive cap below the declaration proves a selected read is used.
                cap = 900 if mode else 2000
                argv = [args.cli, "--format", "tsv", "query", "run", sql,
                        "--database", database, "--native-rows", cap, *mode]
                actual = command(argv)
                if actual.returncode or actual.stdout != expected:
                    raise RuntimeError(f"Boolean selection differs: {argv}\n{actual.stderr}")
                compared_rows += len(expected_ids)
                observations.append({
                    "database": database.name, "mode": mode, "value": value,
                    "command": recorded_command(argv),
                    "rows": len(expected_ids), "stdout_sha256": hashlib.sha256(
                        actual.stdout.encode()).hexdigest(),
                })
        for flags, phrase in [
            (["--native-rows", "900"], "limit"),
            (["--auto-index", "--native-rows", "1"], "limit"),
            (["--auto-index", "--output-bytes", "1"], "output"),
        ]:
            sql = "SELECT ID FROM BooleanIndexed WHERE Flag=True"
            argv = [args.cli, "--format", "tsv", "query", "run", sql,
                    "--database", database, *flags]
            actual = command(argv)
            if actual.returncode == 0 or actual.stdout or phrase not in actual.stderr.lower():
                raise RuntimeError(f"Expected buffered {phrase} refusal: {actual.stderr}")
            observations.append({
                "database": database.name, "refusal": phrase, "flags": flags,
                "command": recorded_command(argv), "stderr": actual.stderr,
            })

    if hashes != {relative(path): digest(path) for path in pinned}:
        raise RuntimeError("Pinned input, binary or source changed during replay")
    report = {
        "date": "2026-10-25", "scope": "Independent getter-model engineering comparison; Access unrun",
        "controls": len(observations), "successful_queries": 16, "buffered_refusals": 6,
        "compared_rows": compared_rows, "getters": getters,
        "pins": hashes, "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {len(observations)} controls; all {len(hashes)} pins unchanged")


if __name__ == "__main__":
    main()
