"""Independent ordinary-getter model for explicit calculated dependency previews."""

import argparse
import base64
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_progressive_queries import display


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
FORMULAS = {
    "Twice": "[Input]*2", "Total": "[twICE]+[Input]", "Diamond": "[Twice]+[Total]",
    "Alias": "[Total]", "Constant": "7", "ConstantAlias": "[Constant]+1",
    "Caption": "[Label]&':'&[Label]", "Title": "[Caption]&'!'",
    "Divide": "[Input]\\([Input]-2)", "LateError": "[Divide]+1",
}
SUCCESSFUL = tuple(name for name in FORMULAS if name not in ("Divide", "LateError"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decode(value):
    if value == "N":
        return None
    if value.startswith("I"):
        return int(value[1:])
    assert value.startswith("T")
    return base64.b64decode(value[1:]).decode("utf-8")


def getters(path):
    tables = {}
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "TABLE":
            tables[fields[1]] = {"declared_rows": int(fields[2]), "columns": [], "rows": []}
        elif fields[0] == "COLUMN":
            name, kind, calculated, expression, result_type = fields[2:]
            if name in FORMULAS:
                assert calculated == "true" and decode(expression) == FORMULAS[name], fields
                assert kind == ("TEXT" if name in ("Caption", "Title") else "LONG")
                assert result_type == ("10" if kind == "TEXT" else "4")
            else:
                assert calculated == "false" and expression == "N" and result_type == "null"
            tables[fields[1]]["columns"].append(name)
        else:
            assert fields[0] == "ROW"
            table = tables[fields[1]]
            table["rows"].append(dict(zip(table["columns"], map(decode, fields[2:]), strict=True)))
    assert tables["Chains"]["declared_rows"] == len(tables["Chains"]["rows"]) == 5
    assert tables["EmptyChains"]["declared_rows"] == len(tables["EmptyChains"]["rows"]) == 0
    assert len(tables["Chains"]["columns"]) == 13
    return tables


def model(column, row):
    value = row["Input"]
    text = row["Label"] or ""
    if column == "Constant":
        return 7
    if column == "ConstantAlias":
        return 8
    if column == "Caption":
        return text + ":" + text
    if column == "Title":
        return text + ":" + text + "!"
    if value is None:
        return None
    return value * {"Twice": 2, "Total": 3, "Diamond": 5, "Alias": 3}[column]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "source_manifest", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.source_manifest) == args.manifest_sha256
    source_hashes = dict(line.split("\0") for line in args.source_manifest.read_text().splitlines())
    paths = {args.cli, args.jar, args.source_manifest, Path(__file__)}
    paths.update(path for path in args.directory.iterdir() if path.is_file())
    generation = json.loads((args.directory / "generation.json").read_text())
    compiled = ROOT / generation["compiled_class"]
    assert digest(compiled) == generation["compiled_class_sha256"]
    paths.add(compiled)
    paths.update(ROOT / "docs/testing" / name for name in (
        "CalculatedDependencyFixture.java", "compare_progressive_queries.py",
    ))
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths)}
    for name, expected in source_hashes.items():
        assert digest(ROOT / name) == expected, name
    hashes.update(source_hashes)
    observations = []
    matched_cells = 0
    preview_cache_differences = 0

    def run(label, database, table, column, expected=None, refusal=None, extra=(), cached=False):
        nonlocal matched_cells
        command = [str(args.cli), "--format", "tsv", "query"]
        if cached:
            command.extend(["run", "--stored-calculations"])
        else:
            command.append("preview-calculation")
        command.extend(["--database", str(database), "--number-format", "invariant"])
        command.extend(extra)
        if cached:
            command.append("SELECT ID,Twice,Total,Diamond,Alias,Constant,ConstantAlias,Caption,Title "
                           "FROM Chains")
        else:
            command.extend([table, column])
        result = subprocess.run(command, capture_output=True, text=True, timeout=60)
        if refusal is not None:
            assert result.returncode != 0 and not result.stdout, (label, result.stdout, result.stderr)
            assert refusal in result.stderr, (label, refusal, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (label, result.stderr)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert actual == expected, (label, actual, expected)
            matched_cells += sum(map(len, expected[1:]))
        observations.append({"label": label, "command": command, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr,
                             "expected_refusal": refusal})

    for profile in PROFILES:
        database = args.directory / (profile + ".accdb")
        tables = getters(database.with_suffix(".accdb.getters.tsv"))
        for name, table in tables.items():
            for column in SUCCESSFUL:
                values = [model(column, row) for row in table["rows"]]
                expected = [["Recomputed"], *[[display(value)] for value in values]]
                run(f"{profile}:{name}:{column}", database, name, column, expected)
                preview_cache_differences += sum(
                    value != row[column] for value, row in zip(values, table["rows"], strict=True)
                )
        for column in ("Divide", "LateError"):
            run(profile + ":late:" + column, database, "Chains", column, refusal="division by zero")
        for option, message in (("--native-rows", "resource limit"),
                                ("--native-steps", "resource limit"),
                                ("--execution-steps", "resource limit"),
                                ("--output-bytes", "output limit")):
            run(profile + ":quota:" + option, database, "Chains", "Diamond",
                refusal=message, extra=(option, "1"))
        for column in ("Input", "Missing"):
            run(profile + ":ordinary-or-missing:" + column, database, "Chains", column,
                refusal="not supported" if column == "Input" else "native column has no binding")
        names = ["ID", *SUCCESSFUL]
        cached = [names, *[[display(row[name]) for name in names] for row in tables["Chains"]["rows"]]]
        run(profile + ":stored-caches", database, None, None, cached, cached=True)
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    report = {"scope": "Original table-calculation dependency getter/model engineering evidence; "
                       "natural recomputed carriers, no native intermediate conversion/freshness claim.",
              "controls": len(observations), "matched_cells": matched_cells,
              "preview_values_differing_from_raw_caches": preview_cache_differences,
              "source_files_verified_before_after": len(source_hashes),
              "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "matched_cells": matched_cells,
                      "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
