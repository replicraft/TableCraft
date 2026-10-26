"""Independent getter-model replay for explicit typed bare multivalue query output."""
import argparse
import csv
import io
import json
import math
from pathlib import Path
import struct
import subprocess

from compare_browsing import display
from compare_ignored_null_indexes import ROOT, digest, pin

PROFILES = ("V2007", "V2016", "V2019")
COLUMNS = ("Bytes", "Shorts", "Ints", "Singles", "Doubles", "Guids", "Decimals", "Texts")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ORIGINAL_SHA256 = {
    "V2007": "583d4b99abf504cd8b1bae72423cffc02390bdbd2e46fb8ba1667abf7aaab0da",
    "V2016": "ef784999fcb08b6e50a875e05ba702259d900a2ff9d1c79f28a64dec64457c58",
    "V2019": "3b53417c687f0e4096f5a584bc134b74ece7ae2569fcd847b520231f2e45e295",
}


def source_objects(revision):
    roots = [
        "crates/query/tablecraft-query/src",
        "crates/backends/tablecraft-access/src/source",
        "apps/tablecraft-cli/src/commands/query",
    ]
    listed = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", revision, "--", *roots],
        cwd=ROOT, text=True, timeout=30,
    ).splitlines()
    additional = {
        "crates/query/tablecraft-query/src/lib.rs",
        "crates/query/tablecraft-query/src/output_types.rs",
        "crates/query/tablecraft-query/src/plan.rs",
        "crates/query/tablecraft-query/src/binding/relation.rs",
        "crates/query/tablecraft-query/src/binding/relation/table.rs",
        "crates/query/tablecraft-query/src/binding/result_types.rs",
        "crates/query/tablecraft-query/src/execution/result.rs",
        "crates/query/tablecraft-query/src/execution/retrieval.rs",
        "apps/tablecraft-cli/src/commands/query/results.rs",
        "apps/tablecraft-cli/src/commands/query/retrieval.rs",
        "apps/tablecraft-cli/src/commands/query/run_args.rs",
    }
    selected = [
        path for path in listed
        if path.endswith(".rs")
        and ("collection" in path or "multivalue" in path or path in additional)
    ]
    assert selected and additional.issubset(selected)
    result = {}
    for path in selected:
        data = subprocess.check_output(
            ["git", "show", f"{revision}:{path}"], cwd=ROOT, timeout=30,
        )
        result[path] = {"bytes": len(data), "sha256": digest(data)}
    return result


def getter_records(path):
    records = list(csv.reader(io.StringIO(path.read_text()), delimiter="\t"))
    assert len(records) == 4
    assert [row[3] for row in records] == [f"I32:{value}" for value in range(1, 5)]
    return {int(row[3].split(":")[1]): row[4:] for row in records}


def scalar_matches(actual, expected):
    if expected.startswith(("F", "D")):
        width = ">f" if expected.startswith("F") else ">d"
        value = struct.unpack(width, bytes.fromhex(expected[1:]))[0]
        parsed = float(actual)
        if math.isnan(value):
            return math.isnan(parsed)
        # CLI display need not expose NaN payloads. Finite values round trip
        # to their stored width; signed-zero spelling remains its own contract.
        return struct.pack(width, parsed) == struct.pack(width, value)
    return actual == display(expected)


def list_matches(actual, expected):
    if any(value.startswith(("F", "D")) for value in expected):
        assert actual.startswith("[") and actual.endswith("]")
        cells = actual[1:-1].split(", ") if actual != "[]" else []
        return len(cells) == len(expected) and all(
            scalar_matches(cell, value) for cell, value in zip(cells, expected)
        )
    return actual == "[" + ", ".join(display(value) for value in expected) + "]"


def compare_table(text, headings, expected_rows):
    actual = list(csv.reader(io.StringIO(text), delimiter="\t"))
    assert actual and actual[0] == headings, actual[:1]
    assert len(actual) == len(expected_rows) + 1
    for row, expected in zip(actual[1:], expected_rows):
        assert len(row) == len(expected)
        for cell, value in zip(row, expected):
            if isinstance(value, list):
                matches = list_matches(cell, value)
            else:
                matches = scalar_matches(cell, value)
            assert matches, (cell, value)


def list_rows(model, column, identifiers):
    return [[f"I32:{identifier}", model[column][identifier]] for identifier in identifiers]


def cases(model):
    identifiers = list(range(1, 5))
    matrix = [
        [f"I32:{identifier}"] + [model[column][identifier] for column in COLUMNS]
        for identifier in identifiers
    ]
    yield (
        "all-carriers", "SELECT ID," + ",".join(COLUMNS) + " FROM Multivalues ORDER BY ID",
        ["ID", *COLUMNS], matrix, None, [],
    )
    yield (
        "aliases", "SELECT ID AS Parent,Texts AS Choices FROM Multivalues ORDER BY ID",
        ["Parent", "Choices"], list_rows(model, "Texts", identifiers), None, [],
    )
    yield (
        "derived", "SELECT Q.ID,Q.Ints FROM (SELECT ID,Ints FROM Multivalues) Q ORDER BY Q.ID",
        ["ID", "Ints"], list_rows(model, "Ints", identifiers), None, [],
    )
    yield (
        "top", "SELECT TOP 2 ID,Texts FROM Multivalues ORDER BY ID DESC",
        ["ID", "Texts"], list_rows(model, "Texts", [4, 3]), None, [],
    )
    for name, predicate, wanted in [
        ("hidden-value", "Ints.Value=0", [
            identifier for identifier in identifiers if "I32:0" in model["Ints"][identifier]
        ]),
        ("hidden-independent-members", "Ints.Value=0 AND Ints.Value=1", [
            identifier for identifier in identifiers
            if "I32:0" in model["Ints"][identifier] and "I32:1" in model["Ints"][identifier]
        ]),
    ]:
        yield (
            name, f"SELECT ID,Ints FROM Multivalues WHERE {predicate} ORDER BY ID",
            ["ID", "Ints"], list_rows(model, "Ints", wanted), None, [],
        )
    for name, predicate, wanted in [
        ("empty", "ID IN (3,4)", [
            identifier for identifier in identifiers if not model["Texts"][identifier]
        ]),
        ("present-null-member", "ID IN (1,2)", [
            identifier for identifier in identifiers if model["Texts"][identifier]
        ]),
    ]:
        yield (
            name, f"SELECT ID,Texts FROM Multivalues WHERE {predicate} ORDER BY ID",
            ["ID", "Texts"], list_rows(model, "Texts", wanted), None, [],
        )
    expanded = [
        [f"I32:{identifier}", member]
        for identifier in identifiers
        for member in (model["Texts"][identifier] or ["N"])
    ]
    yield (
        "existing-value-expansion", "SELECT ID,Texts.Value AS V FROM Multivalues ORDER BY ID",
        ["ID", "V"], expanded, None, [],
    )
    for name, sql, reason, flags in [
        ("distinct-refusal", "SELECT DISTINCT Ints FROM Multivalues", "not implemented", []),
        ("list-order-refusal", "SELECT Ints FROM Multivalues ORDER BY Ints", "not implemented", []),
        ("mixed-value-refusal", "SELECT Ints,Ints.Value FROM Multivalues", "not implemented", []),
        (
            "union-refusal", "SELECT Ints FROM Multivalues UNION ALL SELECT Ints FROM Multivalues",
            "not implemented", [],
        ),
        ("member-limit", "SELECT Items FROM Overflow", "structural limit", []),
        ("scalar-nonfinite-control", "SELECT Singles.Value FROM Multivalues", "non-finite", []),
        ("output-limit", "SELECT ID,Texts FROM Multivalues", "output", ["--output-bytes", "1"]),
    ]:
        yield name, sql, [], [], reason, flags


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--originals", type=Path, required=True)
    parser.add_argument("--compiled-revision", required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists() and not args.log.exists()
    assert digest(args.cli.read_bytes()) == args.cli_sha256
    compiled_revision = subprocess.check_output(
        ["git", "rev-parse", args.compiled_revision], cwd=ROOT, text=True, timeout=30,
    ).strip()
    compiled_sources = source_objects(compiled_revision)
    jar = ROOT / ".cache/tablecraft-fixtures/jackcess-5.0.4.jar"
    assert digest(jar.read_bytes()) == JAR_SHA256
    artifacts = [
        args.cli, jar, Path(__file__), ROOT / "docs/testing/compare_browsing.py",
        ROOT / "docs/testing/compare_ignored_null_indexes.py",
        ROOT / "docs/testing/MultivalueFixture.java", ROOT / "docs/testing/MultivalueOracle.java",
        args.originals / "producer-commands.json", args.originals / "getter-commands.json",
    ]
    for profile in PROFILES:
        original = args.originals / f"{profile}.accdb"
        assert digest(original.read_bytes()) == ORIGINAL_SHA256[profile]
        artifacts.append(original)
        artifacts.extend(args.originals / f"{profile}-{column}-getter.stdout" for column in COLUMNS)
    before = [pin(path) for path in artifacts]
    observations = []
    rows_compared = 0
    cells_compared = 0
    with args.log.open("x", encoding="utf-8") as log:
        for profile in PROFILES:
            model = {
                column: getter_records(args.originals / f"{profile}-{column}-getter.stdout")
                for column in COLUMNS
            }
            for label, sql, headings, expected, refusal, flags in cases(model):
                command = [
                    str(args.cli), "--format", "tsv", "query", "run", sql,
                    "--database", str(args.originals / f"{profile}.accdb"),
                    "--multivalues", "--native-bytes", "67108864", *flags,
                ]
                result = subprocess.run(command, capture_output=True, text=True, timeout=90)
                disagreement = None
                try:
                    if refusal:
                        assert result.returncode != 0 and not result.stdout
                        assert refusal in result.stderr
                    else:
                        assert result.returncode == 0 and not result.stderr
                        compare_table(result.stdout, headings, expected)
                        rows_compared += len(expected)
                        cells_compared += sum(map(len, expected))
                except AssertionError as failure:
                    disagreement = str(failure) or "Unexpected exit/output/error class"
                observation = {
                    "profile": profile, "case": label, "command": command,
                    "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                    "agrees": disagreement is None, "disagreement": disagreement,
                }
                log.write(json.dumps(observation) + "\n")
                observations.append(observation)
    assert [pin(path) for path in artifacts] == before
    report = {
        "scope": "Original getter-derived typed list output and existing scalar .Value controls; no lookup display or native Access SQL qualification",
        "controls": len(observations), "disagreements": sum(not row["agrees"] for row in observations),
        "rows_compared": rows_compared, "cells_compared": cells_compared,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log),
        "compiled_revision": compiled_revision,
        "selected_compiled_source_objects": compiled_sources,
        "source_pin_model": "Selected immutable git objects at the supplied frozen-build revision; no live moving source inventory is pinned.",
        "observations": observations,
        "limitations": [
            "Bracketed list cells use TableCraft display within CSV-quoted TSV; this is neither JSON nor a native Access collection representation.",
            "NaN text establishes NaN only, not payload/sign bits. Typed API tests independently retain exact stored IEEE representations.",
            "These fresh original inputs have their own hashes; historical registered corpus records remain unchanged.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({
        "controls": report["controls"],
        "disagreements": report["disagreements"],
        "report": pin(args.report),
    }))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
