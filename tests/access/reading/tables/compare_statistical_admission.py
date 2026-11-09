"""Numeric-only statistical inputs and explicit DATE serial conversion.

Jackcess reads immutable native fields; Fraction models numeric statistics.
Only TableCraft executes SQL. Native Access behavior remains unqualified.
"""
import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import math
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


FUNCTIONS = [("Var", "DVar"), ("VarP", "DVarP"),
             ("StDev", "DStDev"), ("StDevP", "DStDevP")]


def moments(values):
    count = len(values)
    if not count:
        return [None] * len(FUNCTIONS)

    mean = sum(values, Fraction()) / count
    squares = sum(((value - mean) ** 2 for value in values), Fraction())
    sample = squares / (count - 1) if count > 1 else None
    population = squares / count
    return [sample, population if count > 1 else None,
            math.sqrt(sample) if sample is not None else None,
            math.sqrt(population)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(sql, options=(), expected=None):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, "--format", "tsv", *map(str, options)],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert "expression type mismatch" in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert len(rows) == 2 and len(rows[1]) == len(expected), (sql, rows)
            for actual, wanted in zip(rows[1], expected):
                if wanted is None:
                    assert actual == "NULL", (sql, actual)
                else:
                    assert math.isclose(float(actual), float(wanted), rel_tol=2e-13,
                                        abs_tol=1e-300), (sql, actual, wanted)
        controls += 1

    for function, _ in FUNCTIONS:
        for expression in ["#2024-01-01#", "'1.25'", "CVErr(5)"]:
            query(f"SELECT 1,{function}({expression})")
        query(f"SELECT {function}(NULL)", expected=[None])

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "--date-bits", "IndexedRows",
        ], text=True, encoding="utf-8")
        records = [line.split("\t") for line in output.splitlines()]
        assert len(records) == 20000
        values = []
        for row in records:
            tag = row[9]
            if tag == "N":
                continue
            assert tag.startswith("Y64:")
            serial = struct.unpack(">d", bytes.fromhex(tag[4:]))[0]
            values.append(Fraction.from_float(serial))
        expected = moments(values)
        options = ["--database", path, "--native-bytes", "67108864",
                   "--native-rows", "100000", "--native-steps", "10000000",
                   "--execution-steps", "10000000"]
        for index in [None, "IdAsc", "IdDesc"]:
            configured = options + (["--index", "IndexedRows", index] if index else [])
            for (function, domain), wanted in zip(FUNCTIONS, expected):
                query(f"SELECT 1,{function}(Stamp) FROM IndexedRows", configured)
                query(f"SELECT 1,{domain}('Stamp','IndexedRows')", configured)
                query(f"SELECT {function}(CDbl(Stamp)) FROM IndexedRows WHERE Stamp IS NOT NULL", configured, [wanted])
                query(f"SELECT {domain}('CDbl(Stamp)','IndexedRows','Stamp IS NOT NULL')", configured, [wanted])
        assert path.read_bytes() == before

    print(f"Passed {controls} statistical admission/Fraction/native-field controls; native Access unrun")


if __name__ == "__main__":
    main()
