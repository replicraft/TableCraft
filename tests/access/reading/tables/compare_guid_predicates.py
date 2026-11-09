"""Replay GUID/structure-byte predicates over unchanged native fields and indexes.

Python UUID and independent read-only getters supply identities. Only TableCraft
executes SQL; this is not native Access comparison or byte-array qualification.
"""

import argparse
import csv
import hashlib
import io
import itertools
import json
from pathlib import Path
import subprocess
import uuid

from compare_declared_numeric_text import INPUTS, JAR_SHA256
from compare_fixed_binary_keys import digest
from compare_large_indexes import INPUT_HASHES, registered_input_hash

ROOT = Path(__file__).resolve().parents[2]
CANDIDATE_SHA256 = "2be6ed81c1839ce7235bf29ce0b6d6e8f5d6aea70da5ac4e20109ff62bf5b9ed"
BASELINE_SHA256 = "5a73870b888d58a22314096e761ec3ddec8cfc4ea6bbf7a7e5acda868b46d10c"
MAX_COMMANDS = 400
SOURCE_FILES = [
    "crates/query/tablecraft-expressions/src/evaluation/comparison_values.rs",
    "crates/query/tablecraft-expressions/src/evaluation/comparison_values/guid.rs",
    "crates/query/tablecraft-expressions/src/comparison.rs",
    "crates/query/tablecraft-query/src/execution/constraint.rs",
]


def getter(jar, script, database, *arguments):
    result = subprocess.run(
        ["java", "-Xmx256m", "-cp", str(jar), str(ROOT / "docs/testing" / script),
         str(database), *arguments],
        capture_output=True, text=True, encoding="utf-8", timeout=120, check=True,
    )
    if result.stderr:
        raise ValueError("Unexpected getter diagnostics")
    return result.stdout


def scalar_rows(text):
    rows = []
    for line in text.splitlines():
        table, *fields = line.split("\t")
        if table != "ScalarSamples" or len(fields) != 14 or not fields[0].startswith("I32:"):
            raise ValueError("Unexpected scalar getter shape")
        guid = fields[11]
        if guid != "N" and not guid.startswith("G"):
            raise ValueError("Unexpected GUID getter carrier")
        rows.append((int(fields[0][4:]), None if guid == "N" else uuid.UUID(hex=guid[1:])))
    if [identity for identity, _ in rows] != [1, 2, 3]:
        raise ValueError("Unexpected original scalar identities")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "baseline", "jar", "directory", "indexes", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    if (digest(args.cli), digest(args.baseline), digest(args.jar)) != (
        CANDIDATE_SHA256, BASELINE_SHA256, JAR_SHA256
    ):
        raise ValueError("Executable/reference fingerprint differs")
    for name, expected in INPUTS.items():
        if digest(args.directory / name) != expected:
            raise ValueError("Original scalar fingerprint differs")
    large_inputs = []
    for suffix, original in INPUT_HASHES.items():
        database = args.indexes / ("index-" + suffix)
        registered_input_hash(database, original)
        large_inputs.append(database)
    revision = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    sources = {}
    for name in SOURCE_FILES:
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"], check=True, capture_output=True,
        ).stdout
        sources[name] = hashlib.sha256(content).hexdigest()
        if digest(ROOT / name) != sources[name]:
            raise ValueError("Selected source differs from revision")
    tools = [args.cli, args.baseline, args.jar, Path(__file__),
             ROOT / "docs/testing/TableDataOracle.java", ROOT / "docs/testing/IndexRowsOracle.java",
             ROOT / "docs/testing/compare_declared_numeric_text.py",
             ROOT / "docs/testing/compare_fixed_binary_keys.py",
             ROOT / "docs/testing/compare_numeric_keys.py",
             ROOT / "docs/testing/compare_large_indexes.py",
             *(args.directory / name for name in INPUTS), *large_inputs]
    pins = {str(path.resolve().relative_to(ROOT)): digest(path) for path in tools}
    report = {
        "date": "2026-10-25", "revision": revision, "scope": __doc__,
        "immutable_before": pins, "selected_source_sha256": sources,
        "getters": {}, "observations": [], "complete": False,
    }

    def run(database, label, sql, wanted, declared=False, baseline=False, stream=False,
            index=None, parameters=(), refusal=None, large=False):
        if len(report["observations"]) >= MAX_COMMANDS:
            raise ValueError("CLI observation ceiling exceeded")
        binary = args.baseline if baseline else args.cli
        command = [str(binary), "--format", "tsv", "query", "run", sql,
                   "--database", str(database), "--binding-depth", "32"]
        if declared:
            command.append("--declared-arithmetic")
        if stream:
            command.append("--stream")
        if index:
            command.extend(["--index", "IndexedRows", index])
        for parameter in parameters:
            command.extend(["--param", parameter])
        if large:
            command.extend(["--native-bytes", "67108864", "--native-rows", "100000",
                            "--native-steps", "10000000", "--execution-steps", "10000000"])
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        item = {
            "database": database.name, "case": label, "declared": declared,
            "baseline": baseline, "stream": stream, "index": index,
            "parameters": list(parameters), "large_allowances": large,
            "sql_sha256": hashlib.sha256(sql.encode()).hexdigest(),
            "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr.encode()).hexdigest(),
        }
        report["observations"].append(item)
        if refusal:
            if result.returncode != 2 or result.stdout or refusal not in result.stderr:
                raise AssertionError((item, result.stderr))
            item.update(refusal=refusal, rows=0, cells=0)
            return
        if result.returncode or result.stderr:
            raise AssertionError((item, result.stderr, sql))
        table = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
        if table != wanted:
            raise AssertionError((item, table, wanted))
        item.update(rows=len(table) - 1, cells=sum(len(row) for row in table[1:]))

    try:
        for name in INPUTS:
            database = args.directory / name
            text = getter(args.jar, "TableDataOracle.java", database, "ScalarSamples")
            rows = scalar_rows(text)
            report["getters"][name] = {
                "stdout_sha256": hashlib.sha256(text.encode()).hexdigest(), "rows": len(rows),
                "values": [[identity, None if value is None else str(value)] for identity, value in rows],
            }
            targets = [value for _, value in rows if value is not None]
            for target, declared in itertools.product(targets, [False, True]):
                matching = [[str(identity)] for identity, value in rows if value == target]
                different = [[str(identity)] for identity, value in rows if value is not None and value != target]
                binary = f"GUIDFromString('{target}')"
                predicates = {
                    "equal": (f"G={binary}", matching),
                    "reverse": (f"{binary}=G", matching),
                    "not-equal": (f"G<>{binary}", different),
                    "list": (f"G IN ({binary})", matching),
                    "subquery": (f"G IN (SELECT {binary})", matching),
                    "quantified": (f"G=ANY(SELECT {binary})", matching),
                }
                for label, (predicate, expected) in predicates.items():
                    sql = f"SELECT ID FROM ScalarSamples WHERE {predicate} ORDER BY ID"
                    run(database, f"{target}-{label}", sql, [["ID"], *expected], declared)
                run(database, f"{target}-stream", f"SELECT ID FROM ScalarSamples WHERE G={binary}",
                    [["ID"], *matching], declared, stream=True)
                criterion = f'G=GUIDFromString("{target}")'
                run(database, f"{target}-domain", f"SELECT DCount('*','ScalarSamples','{criterion}') AS N",
                    [["N"], [str(len(matching))]], declared)
                run(database, f"{target}-parameter", "PARAMETERS P TEXT; SELECT ID FROM ScalarSamples WHERE G=GUIDFromString(P) ORDER BY ID",
                    [["ID"], *matching], declared, parameters=[f"P=text:{target}"])
                if not declared:
                    run(database, f"{target}-prior-refusal", f"SELECT ID FROM ScalarSamples WHERE G={binary}",
                        None, baseline=True, refusal="expression type mismatch")
            for label, sql, refusal in [
                ("ordering", f"SELECT ID FROM ScalarSamples WHERE G<GUIDFromString('{targets[0]}')", "expression form is not implemented"),
                ("eager", "SELECT ID FROM ScalarSamples WHERE False AND G=GUIDFromString('bad')", "expression type mismatch"),
                ("wrong-width", "SELECT ID FROM ScalarSamples WHERE G=[Bytes]", "expression type mismatch"),
                ("null-constructor", "SELECT ID FROM ScalarSamples WHERE G=GUIDFromString(NULL)", "invalid use of NULL"),
            ]:
                run(database, label, sql, None, refusal=refusal)
            print(f"Checked {name}", flush=True)
        for database in large_inputs:
            text = getter(args.jar, "IndexRowsOracle.java", database, "IndexedRows", "GuidAsc", "ID", "GuidValue")
            rows = [(int(identity), None if guid == "NULL" else uuid.UUID(guid.strip("{}")))
                    for identity, guid in (line.split("\t") for line in text.splitlines())]
            if sorted(identity for identity, _ in rows) != list(range(1, 20001)):
                raise ValueError("Unexpected indexed corpus identities")
            target = next(value for _, value in rows if value is not None)
            expected = [["ID"], *[[str(identity)] for identity, value in sorted(rows) if value == target]]
            report["getters"][database.name] = {
                "stdout_sha256": hashlib.sha256(text.encode()).hexdigest(), "rows": len(rows),
                "target": str(target), "matching_ids": [int(row[0]) for row in expected[1:]],
            }
            sql = f"SELECT ID FROM IndexedRows WHERE GuidValue=GUIDFromString('{target}') ORDER BY ID"
            for index, baseline in itertools.product([None, "GuidAsc", "GuidDesc"], [False, True]):
                run(database, "indexed-stored-field", sql, expected, index=index, baseline=baseline,
                    refusal="expression type mismatch" if baseline else None, large=True)
            print(f"Checked {database.name}: sequential and both required GUID indexes", flush=True)
        report["immutable_after"] = {name: digest(ROOT / name) for name in pins}
        if report["immutable_after"] != pins:
            raise ValueError("Pinned original, executable or tool changed")
        if any(digest(ROOT / name) != pin for name, pin in sources.items()):
            raise ValueError("Selected source changed")
        report["complete"] = True
    finally:
        report["cli_commands"] = len(report["observations"])
        report["refusals"] = sum("refusal" in item for item in report["observations"])
        report["compared_rows"] = sum(item.get("rows", 0) for item in report["observations"])
        report["compared_cells"] = sum(item.get("cells", 0) for item in report["observations"])
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {report['cli_commands']} CLI observations", flush=True)


if __name__ == "__main__":
    main()
