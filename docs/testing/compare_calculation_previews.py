"""Independently model explicit row-local previews from inert ordinary getters.

This qualifies the engineering preview and its separation from stored caches,
not native Access formula evaluation, freshness or result-type conversion.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

from cli_tsv import display_tsv
from compare_browsing import display, escaped
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
ORACLE = RECIPES / "CalculationPreviewOracle.java"
PROFILES = ("V2016", "V2019")


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=120
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def getter(jar, path, table, column, dependency):
    output = execute(
        "java", "-Dstdout.encoding=UTF-8", "-cp", jar, ORACLE,
        path, table, column, dependency,
    )
    declaration, *lines = output.splitlines()
    kind, expression = declaration.split("\t")
    assert kind == "D" and expression.startswith("T")
    expression = bytes.fromhex(expression[1:]).decode("utf-8")
    rows = []
    for ordinal, line in enumerate(lines):
        tag, index, cached, ordinary = line.split("\t")
        assert tag == "R" and int(index) == ordinal
        rows.append((cached, ordinary))
    return expression, rows, hashlib.sha256(output.encode()).hexdigest()


def model(ordinary, operation):
    if ordinary == "N":
        return "NULL"
    if operation == "later":
        assert ordinary.startswith("T")
        text = bytes.fromhex(ordinary[1:]).decode("utf-8") + "later"
        return '"' + escaped(text) + '"'

    assert ordinary.startswith("I32:")
    value = int(ordinary.split(":")[1])
    if operation == "twice":
        return str(value * 2)
    if operation == "prefix":
        return '"prefix' + str(value) + '"'
    divisor = {"ten_thousand": 10000, "half": 2}[operation]
    return float(Fraction(value, divisor))


def equal(actual, expected):
    if isinstance(expected, float):
        # Compare exact binary64 values independently of shortest-decimal spelling.
        try:
            return struct.pack(">d", float(actual)) == struct.pack(">d", expected)
        except ValueError:
            return False
    return actual == expected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    names = (
        "cli", "jar", "decimal_directory", "memo_directory",
        "evolution_directory", "simple_directory", "report",
    )
    for name in names:
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite an evidence report"
    assert digest(args.jar) == JACKCESS_SHA256

    source_paths = [
        *sorted((ROOT / "crates/backends/tablecraft-access/src/source/calculation").glob("*.rs")),
        ROOT / "crates/backends/tablecraft-access/src/source/calculation.rs",
        ROOT / "apps/tablecraft-cli/src/commands/query/preview_calculation.rs",
        ORACLE, Path(__file__), RECIPES / "cli_tsv.py",
        RECIPES / "compare_browsing.py", RECIPES / "compare_calculated_decimals.py",
        RECIPES / "float_model.py",
        *[RECIPES / name for name in (
            "CalculatedDecimalFixture.java", "CalculatedMemoFixture.java",
            "AdvancedEvolutionFixture.java", "CalculatedStorageFixture.java",
            "CalculatedScalarFixture.java",
        )],
    ]
    # Directory, file prefix, table, target, ordinary dependency, formula, model.
    definitions = (
        (args.decimal_directory, "", "CalculatedDecimals", "Amount", "ID",
         "[ID]/10000", "ten_thousand"),
        (args.memo_directory, "", "CalculatedMemos", "Wide", "ID",
         '"prefix" & [ID]', "prefix"),
        (args.memo_directory, "", "CalculatedMemos", "Compact", "ID",
         '"prefix" & [ID]', "prefix"),
        (args.evolution_directory, "", "Evolution", "Cached", "ID",
         "[ID]*2", "twice"),
        (args.evolution_directory, "", "Evolution", "LaterMemo", "Label",
         '[Label] & "later"', "later"),
        (args.evolution_directory, "", "Evolution", "Amount", "ID",
         "[ID]/10000", "ten_thousand"),
        (args.simple_directory, "Storage-", "Calculations", "Twice", "Score",
         "[Score]*2", "twice"),
        (args.simple_directory, "Scalar-", "CalculatedScalars", "Number", "Id",
         "[Id]/2", "half"),
        (args.simple_directory, "Scalar-", "CalculatedScalars", "Amount", "Id",
         "[Id]/10000", "ten_thousand"),
        (args.simple_directory, "Scalar-", "CalculatedScalars", "Label", "Id",
         '"prefix" & [Id]', "prefix"),
    )
    cases = []
    for profile in PROFILES:
        for directory, prefix, *contract in definitions:
            cases.append((directory / f"{prefix}{profile}.accdb", *contract))

    inputs = {*source_paths, args.cli, args.jar, *(case[0] for case in cases)}
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(inputs)}
    observations = []
    refused_declarations = []
    controls = 0
    recomputed_cells = 0
    stale_cells = 0

    def preview(path, table, column, *extra):
        output = execute(
            args.cli, "--format", "tsv", "query", "preview-calculation",
            "--database", path, table, column, "--number-format", "invariant", *extra,
        )
        heading, *values = display_tsv(output).splitlines()
        assert heading == "Recomputed"
        return values

    def refused(path, table, column, *extra):
        result = subprocess.run(
            list(map(str, (
                args.cli, "--format", "tsv", "query", "preview-calculation",
                "--database", path, table, column, *extra,
            ))), capture_output=True, timeout=120,
        )
        assert result.returncode == 2 and not result.stdout, result.stderr

    for path, table, column, dependency, formula, operation in cases:
        expression, rows, oracle_hash = getter(args.jar, path, table, column, dependency)
        assert expression == formula, (table, column, expression, formula)
        expected = [model(ordinary, operation) for _, ordinary in rows]
        actual = preview(path, table, column)
        assert len(actual) == len(expected)
        assert all(equal(value, wanted) for value, wanted in zip(actual, expected)), (
            path, table, column, actual, expected,
        )
        controls += 1
        recomputed_cells += len(expected)
        different = sum(
            not equal(display(cached), wanted)
            for (cached, _), wanted in zip(rows, expected)
        )
        stale_cells += different
        observations.append({
            "file": str(path.resolve().relative_to(ROOT)), "table": table,
            "column": column, "expression": expression, "ordinary_dependency": dependency,
            "rows": len(rows), "cached_values_differing": different,
            "ordinary_nulls": sum(ordinary == "N" for _, ordinary in rows),
            "cached_nulls": sum(cached == "N" for cached, _ in rows),
            "oracle_sha256": oracle_hash,
        })

    for profile in PROFILES:
        path = args.simple_directory / f"Storage-{profile}.accdb"
        assert preview(path, "EmptyCalculations", "Twice") == []
        controls += 1
        for table, column, extra in (
            ("Calculations", "Score", ()),
            ("Calculations", "Missing", ()),
            ("Calculations", "Twice", ("--native-bytes", "1")),
            ("Calculations", "Twice", ("--execution-steps", "1")),
            ("Calculations", "Twice", ("--output-bytes", "1")),
        ):
            refused(path, table, column, *extra)
            controls += 1
        scalar = args.simple_directory / f"Scalar-{profile}.accdb"
        for table in ("CalculatedScalars", "EmptyCalculatedScalars"):
            for column, formula in (
                ("Flag", "[Id]>0"),
                ("Moment", "DateSerial(2000,1,[Id])"),
            ):
                expression, rows, oracle_hash = getter(
                    args.jar, scalar, table, column, "Id"
                )
                assert expression == formula
                refused(scalar, table, column)
                refused_declarations.append({
                    "file": str(scalar.resolve().relative_to(ROOT)),
                    "table": table, "column": column, "expression": expression,
                    "rows": len(rows), "oracle_sha256": oracle_hash,
                })
                controls += 1

        # Corrupt a unique cached Decimal wrapper on a disposable copy. Preview
        # still uses ordinary ID; explicit stored reads must refuse the bad cache.
        original = args.decimal_directory / f"{profile}.accdb"
        stored = execute(
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            RECIPES / "CalculatedDecimalFixture.java", "--inspect", original,
        )
        wrapper = bytes.fromhex(stored.splitlines()[-2].split("\t")[-1])
        changed = bytearray(original.read_bytes())
        assert changed.count(wrapper) == 1
        changed[changed.index(wrapper)] = 1
        with tempfile.TemporaryDirectory(prefix="tablecraft-preview-") as directory:
            copy = Path(directory) / "invalid.accdb"
            copy.write_bytes(changed)
            assert preview(copy, "CalculatedDecimals", "Amount") == preview(
                original, "CalculatedDecimals", "Amount"
            )
            result = subprocess.run(
                [str(args.cli), "inspect", str(copy), "--table", "CalculatedDecimals",
                 "--stored-calculation", "Amount"], capture_output=True, timeout=120,
            )
            assert result.returncode == 2 and not result.stdout, result.stderr
            assert copy.read_bytes() == changed
            controls += 2

    assert all(digest(ROOT / name) == value for name, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "recomputed_cells": recomputed_cells, "cached_values_differing": stale_cells,
        "hashes_unchanged": True, "inputs": hashes, "observations": observations,
        "refused_declarations": refused_declarations,
    }
    with args.report.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} preview controls and {recomputed_cells} independently modeled cells")


if __name__ == "__main__":
    main()
