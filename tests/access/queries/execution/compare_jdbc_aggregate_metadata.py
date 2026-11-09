"""Record independent JDBC aggregate metadata separately from TableCraft carriers."""

import argparse
import base64
import csv
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
MAX_RESULT_ROWS = 16
EXPECTED_QUERY_COUNT = 58
PROCESS_TIMEOUT_SECONDS = 60
JAVA_HEAP_ARGUMENT = "-Xmx256m"
CLI_SHA256 = "93ee5e5e27b9b6eea392c5c50566c53b8e0f9de28fef9d7c5c2943b7ff22d4e9"
ARTIFACTS = {
    "ucanaccess-5.2.0.jar": "ed8f33f30b9d08c7dc0bead139180ea70061beea915677ce2fe47e4b7aea2490",
    "jackcess-5.2.0.jar": "9451d6f336cfaa82f5af178d5f6668b933cda4f27e261f29a1faa83630f95775",
    "hsqldb-2.7.4.jar": "5fab2bb4384ac06b762638c8fa2740c944b8d080e4796c0c6c2af8b90dd4e5ad",
}
INPUTS = {
    "Scalar-V2000.mdb": "de2e67a9f0194988d031cc9c981176ae9a76d3bf80599040729b408fc2b7be82",
    "Scalar-V2007.accdb": "4ed9555c239e56f0649040fb9357913e4e38dcbaac5c2cb668614857882922bc",
    "Scalar-V2016.accdb": "95c52665d28869a92764f0958b62ad53e465847e3a0c637829d4ff29c8f5ef37",
    "Scalar-V2019.accdb": "eef6a7d1f23c482333ad5b70b312125bd89799ea168b10a522491eb4b7625190",
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return path.resolve().relative_to(ROOT).as_posix()


def decoded(value):
    return base64.b64decode(value, validate=True).decode("utf-8")


def invoke(arguments, timeout=PROCESS_TIMEOUT_SECONDS):
    result = subprocess.run(
        arguments, cwd=ROOT, capture_output=True, encoding="utf-8",
        check=False, timeout=timeout,
    )
    return {
        "command": arguments, "exit_code": result.returncode,
        "stdout": result.stdout.replace(str(ROOT), "."),
        "stderr": result.stderr.replace(str(ROOT), "."),
    }


def parse_probe(text):
    queries = {}
    for line in text.splitlines():
        cells = line.split("\t")
        kind, label = cells[:2]
        if kind == "QUERY":
            if label in queries:
                raise ValueError("Duplicate query label")
            queries[label] = {"sql": decoded(cells[2]), "metadata": [], "rows": []}
        elif kind == "META":
            queries[label]["metadata"].append({
                "column": int(cells[2]), "jdbc_code": int(cells[3]),
                "jdbc_type_name": decoded(cells[4]), "metadata_class": decoded(cells[5]),
                "precision": int(cells[6]), "scale": int(cells[7]), "nullable": int(cells[8]),
            })
        elif kind == "CELL":
            row = int(cells[2])
            if row > MAX_RESULT_ROWS:
                raise ValueError("Probe row ceiling exceeded")
            rows = queries[label]["rows"]
            while len(rows) < row:
                rows.append([])
            rows[row - 1].append({
                "column": int(cells[3]), "runtime_class": cells[4],
                "value": None if cells[4] == "NULL" else decoded(cells[5]),
            })
        elif kind == "ERROR":
            queries[label]["reference_error"] = {
                "sqlstate": cells[2], "code": int(cells[3]),
                "message": decoded(cells[4]).replace(str(ROOT), "."),
            }
        elif kind == "DONE":
            queries[label]["completed_rows"] = int(cells[2])
        else:
            raise ValueError("Unknown probe output record")
    if len(queries) != EXPECTED_QUERY_COUNT:
        raise ValueError("Expected exactly 58 bounded queries")
    for query in queries.values():
        if "reference_error" not in query and query.get("completed_rows") != len(query["rows"]):
            raise ValueError("Incomplete reference result")
    return queries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--workdir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.workdir.exists() or args.report.exists():
        raise FileExistsError("Work directory and report must be fresh")
    artifacts = [args.artifacts / name for name in ARTIFACTS]
    inputs = [args.inputs / name for name in INPUTS]
    source = ROOT / "docs/testing/JdbcAggregateMetadataProbe.java"
    pinned = [*artifacts, *inputs, args.cli, source, Path(__file__)]
    for directory, expected in [(args.artifacts, ARTIFACTS), (args.inputs, INPUTS)]:
        for name, value in expected.items():
            if digest(directory / name) != value:
                raise ValueError("Reviewed artifact or registered original differs")
    if digest(args.cli) != CLI_SHA256:
        raise ValueError("Frozen CLI differs")
    before = {relative(path): digest(path) for path in pinned}
    classes = args.workdir / "classes"
    classes.mkdir(parents=True)
    classpath = ":".join(relative(path) for path in artifacts)
    compile_record = invoke([
        "javac", "-cp", classpath, "-d", relative(classes), relative(source),
    ])
    if compile_record["exit_code"] != 0:
        raise RuntimeError(compile_record)

    profiles = []
    for original in inputs:
        copy = args.workdir / original.name
        shutil.copy2(original, copy)
        record = invoke([
            "java", JAVA_HEAP_ARGUMENT, "-cp", relative(classes) + ":" + classpath,
            "JdbcAggregateMetadataProbe", relative(copy),
        ])
        (args.workdir / (original.stem + ".json")).write_text(json.dumps(record, indent=2) + "\n")
        if record["exit_code"] != 0 or record["stderr"]:
            raise RuntimeError("Reference process failed; raw attempt preserved")
        queries = parse_probe(record["stdout"])
        for query in queries.values():
            sql = "SELECT VarType(V) AS Carrier,TypeName(V) AS Kind,V FROM (" + query["sql"] + ") Q"
            actual = invoke([
                relative(args.cli), "--format", "tsv", "query", "run", sql,
                "--database", relative(copy), "--number-format", "invariant",
            ])
            query["tablecraft"] = {
                **actual,
                "display_rows": list(csv.reader(io.StringIO(actual["stdout"]), delimiter="\t")),
            }
        if digest(copy) != INPUTS[original.name]:
            raise ValueError("Reference copy changed")
        profiles.append({
            "input": relative(original), "copy": relative(copy),
            "copy_sha256": digest(copy), "queries": queries,
        })

    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Independent UCanAccess/HSQLDB observations, not native Access answers or a pass/fail oracle for differing carrier profiles",
        "compiled_tablecraft_revision": "57140e145eaef59b3de6f8b84eca6852a8526ea8",
        "queries_per_profile": EXPECTED_QUERY_COUNT, "profiles": profiles, "compile": compile_record,
        "reference_query_errors": sum("reference_error" in q for p in profiles for q in p["queries"].values()),
        "tablecraft_query_errors": sum(q["tablecraft"]["exit_code"] != 0 for p in profiles for q in p["queries"].values()),
        "immutable_before": before, "immutable_after": after, "pins_unchanged": before == after,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({key: report[key] for key in [
        "queries_per_profile", "reference_query_errors", "tablecraft_query_errors", "pins_unchanged",
    ]}, indent=2))
    if not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
