"""Check stored GUID inspection and fixed bounds against unchanged native getters.

Only TableCraft executes SQL. Native Access subtype names, array bounds and error
precedence remain unqualified; expected observers follow the explicit Byte view.
"""

import argparse
import csv
import hashlib
import io
import itertools
import json
from pathlib import Path
import subprocess

import compare_guid_predicates as native

ROOT = Path(__file__).resolve().parents[2]
CANDIDATE_SHA256 = "eb8ea6717440d41b9e4a4e159425c71d2d15b5cd2e48ad5c417d5d85654049f4"
BASELINE_SHA256 = "2be6ed81c1839ce7235bf29ce0b6d6e8f5d6aea70da5ac4e20109ff62bf5b9ed"
MAX_COMMANDS = 128
SOURCE_FILES = [
    "crates/query/tablecraft-expressions/src/arrays/bounds.rs",
    "crates/query/tablecraft-expressions/src/functions/inspection/types/input.rs",
    "crates/query/tablecraft-expressions/src/functions/inspection/types/kind.rs",
    "crates/query/tablecraft-expressions/src/evaluation/arrays.rs",
    "crates/query/tablecraft-query/src/execution/arrays.rs",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "baseline", "jar", "directory", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    if (native.digest(args.cli), native.digest(args.baseline), native.digest(args.jar)) != (
        CANDIDATE_SHA256, BASELINE_SHA256, native.JAR_SHA256
    ):
        raise ValueError("Executable/reference fingerprint differs")
    for name, expected in native.INPUTS.items():
        if native.digest(args.directory / name) != expected:
            raise ValueError("Original scalar fingerprint differs")
    revision = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    sources = {}
    for name in SOURCE_FILES:
        data = subprocess.run(["git", "show", f"{revision}:{name}"],
                              capture_output=True, check=True).stdout
        sources[name] = hashlib.sha256(data).hexdigest()
        if native.digest(ROOT / name) != sources[name]:
            raise ValueError("Selected source differs from revision")
    artifacts = [args.cli, args.baseline, args.jar, Path(__file__),
                 ROOT / "docs/testing/compare_guid_predicates.py",
                 ROOT / "docs/testing/compare_declared_numeric_text.py",
                 ROOT / "docs/testing/compare_fixed_binary_keys.py",
                 ROOT / "docs/testing/compare_numeric_keys.py",
                 ROOT / "docs/testing/compare_large_indexes.py",
                 ROOT / "docs/testing/TableDataOracle.java",
                 *(args.directory / name for name in native.INPUTS)]
    pins = {str(path.resolve().relative_to(ROOT)): native.digest(path) for path in artifacts}
    report = {
        "date": "2026-10-25", "revision": revision, "scope": __doc__,
        "immutable_before": pins, "selected_source_sha256": sources,
        "getters": {}, "observations": [], "complete": False,
    }

    def run(database, label, sql, wanted=None, declared=False, stream=False,
            baseline=False, parameters=(), refusal=None):
        if len(report["observations"]) >= MAX_COMMANDS:
            raise ValueError("CLI observation ceiling exceeded")
        binary = args.baseline if baseline else args.cli
        command = [str(binary), "--format", "tsv", "query", "run", sql,
                   "--database", str(database), "--binding-depth", "32"]
        if declared:
            command.append("--declared-arithmetic")
        if stream:
            command.append("--stream")
        for parameter in parameters:
            command.extend(["--param", parameter])
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        item = {
            "database": database.name, "case": label, "declared": declared,
            "stream": stream, "baseline": baseline,
            "parameters": list(parameters), "sql_sha256": hashlib.sha256(sql.encode()).hexdigest(),
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
        actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
        if actual != wanted:
            raise AssertionError((item, actual, wanted))
        item.update(rows=len(actual) - 1, cells=sum(len(row) for row in actual[1:]))

    try:
        for name in native.INPUTS:
            database = args.directory / name
            text = native.getter(args.jar, "TableDataOracle.java", database, "ScalarSamples")
            rows = native.scalar_rows(text)
            present = [identity for identity, value in rows if value is not None]
            missing = [identity for identity, value in rows if value is None]
            report["getters"][name] = {
                "stdout_sha256": hashlib.sha256(text.encode()).hexdigest(), "rows": len(rows),
                "present_ids": present, "null_ids": missing,
            }
            primary = "SELECT ID,TypeName(G) AS N,VarType(G) AS T,IsArray(G) AS A,LBound(G) AS L,UBound(G) AS U,IsEmpty(G) AS E,IsError(G) AS R FROM ScalarSamples WHERE G IS NOT NULL"
            expected = [["ID", "N", "T", "A", "L", "U", "E", "R"],
                        *[[str(identity), '"Byte()"', "8209", "TRUE", "0", "15", "FALSE", "FALSE"] for identity in present]]
            for declared, stream in itertools.product([False, True], [False, True]):
                run(database, "observers", primary, expected, declared, stream)
            run(database, "prior-observers", primary, baseline=True, refusal="expression type mismatch")
            run(database, "prior-isarray", "SELECT IsArray(G) AS A FROM ScalarSamples WHERE G IS NOT NULL",
                [["A"], *[["FALSE"] for _ in present]], baseline=True)
            for declared in [False, True]:
                run(database, "null-observers", "SELECT TypeName(G) AS N,VarType(G) AS T,IsArray(G) AS A FROM ScalarSamples WHERE G IS NULL",
                    [["N", "T", "A"], *[['"Null"', "1", "FALSE"] for _ in missing]], declared)
                run(database, "group", "SELECT TypeName(First(G)) AS N,VarType(G) AS T,LBound(First(G)) AS L,UBound(Last(G)) AS U,Count(*) AS C FROM ScalarSamples WHERE G IS NOT NULL GROUP BY G,VarType(G)",
                    [["N", "T", "L", "U", "C"], *[['"Byte()"', "8209", "0", "15", "1"] for _ in present]], declared)
                run(database, "domain", "SELECT TypeName(DFirst('G','ScalarSamples','ID=1')) AS N,UBound(DLast('G','ScalarSamples','G IS NOT NULL')) AS U",
                    [["N", "U"], ['"Byte()"', "15"]], declared)
                run(database, "subquery", "SELECT LBound((SELECT G FROM ScalarSamples WHERE ID=1)) AS L,VarType((SELECT G FROM ScalarSamples WHERE ID=2)) AS T",
                    [["L", "T"], ["0", "8209"]], declared)
                for identity, value in rows:
                    if value is None:
                        continue
                    run(database, f"parameter-{identity}", "PARAMETERS P GUID; SELECT TypeName(CVar(P)) AS N,UBound(IIf(True,P,P)) AS U",
                        [["N", "U"], ['"Byte()"', "15"]], declared, parameters=[f"P=guid:{value}"])
            for label, sql, refusal in [
                ("null-bound", "SELECT UBound(G) FROM ScalarSamples WHERE G IS NULL", "invalid use of NULL"),
                ("dimension", "SELECT UBound(G,2) FROM ScalarSamples WHERE G IS NOT NULL", "invalid array bounds"),
                ("eager", "SELECT UBound(G,1/0) FROM ScalarSamples WHERE G IS NULL", "division by zero"),
            ]:
                run(database, label, sql, refusal=refusal)
            print(f"Checked {name}", flush=True)
        report["immutable_after"] = {name: native.digest(ROOT / name) for name in pins}
        if report["immutable_after"] != pins:
            raise ValueError("Original, executable or tool changed")
        if any(native.digest(ROOT / name) != expected for name, expected in sources.items()):
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
