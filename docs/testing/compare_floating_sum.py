"""Exact Fraction models for floating sums; Jackcess supplies inert native inputs.

Only TableCraft executes SQL. This does not establish ACE aggregation precision.
"""
import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import itertools
import math
from pathlib import Path
import random
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def exact(values):
    return float(sum((Fraction.from_float(value) for value in values), Fraction()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(sql, expected, options=(), overflow=False):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", "run", sql, "--format", "tsv", *map(str, options)],
                                capture_output=True, text=True, encoding="utf-8", check=False)
        if overflow:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert "numeric overflow" in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert len(rows) == 2 and len(rows[1]) == len(expected), (sql, rows)
            for actual, wanted in zip(rows[1], expected):
                if wanted is None:
                    assert actual == "NULL", (sql, actual)
                else:
                    assert struct.pack(">d", float(actual)) == struct.pack(">d", wanted), (sql, actual, wanted)
        controls += 1

    maximum = float.fromhex("0x1.fffffffffffffp+1023")
    smallest = float.fromhex("0x0.0000000000001p-1022")
    cases = [[], [maximum, maximum], [-maximum, -maximum],
             [maximum, maximum, -maximum], [-maximum, -maximum, maximum],
             [maximum, 1.0, -maximum], [1e308, 1e-300, -1e308],
             [smallest, smallest], [0.0, -0.0],
             [1.0, 2.0 ** -53], [1.0, 2.0 ** -52, 2.0 ** -53]]
    cases.extend(map(list, itertools.permutations([1e308, 1.0, -1e308, 1e-300])))
    generator = random.Random(20261022)
    for _ in range(96):
        values = []
        while len(values) < 8:
            value = struct.unpack(">d", struct.pack(">Q", generator.getrandbits(64)))[0]
            if math.isfinite(value):
                values.append(value)
        cases.append(values)
    for values in cases:
        inputs = " UNION ALL ".join(f"SELECT {value!r} AS Amount" for value in values)
        inputs = inputs or "SELECT CDbl(0) AS Amount WHERE False"
        sql = f"SELECT 42,Sum(Amount) FROM ({inputs}) AS Inputs"
        try:
            expected = exact(values) if values else None
        except OverflowError:
            query(sql, [], overflow=True)
        else:
            query(sql, [42.0, expected])

    # Model Currency conversion as exact scaled decimal -> nearest binary64,
    # independently of TableCraft's converter. IIf keeps heterogeneous inputs
    # inside the aggregate; UNION over the values would pre-coerce the column.
    scaled_cases = [-(2 ** 63), 2 ** 63 - 1, -1, 0, 1, 12500]
    scaled_cases += [generator.randrange(-(2 ** 63), 2 ** 63) for _ in range(32)]
    for scaled, conversion, reverse in itertools.product(scaled_cases, ["CSng", "CDbl"], [False, True]):
        magnitude = abs(scaled)
        literal = f"{'-' if scaled < 0 else ''}{magnitude // 10000}.{magnitude % 10000:04d}"
        condition = "N=2" if reverse else "N=1"
        expression = f"IIf({condition},CCur('{literal}'),{conversion}(0))"
        query(f"SELECT Sum({expression}) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS Inputs",
              [float(Fraction(scaled, 10000))], ["--number-format", "invariant"])

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        records = [line.split("\t") for line in output.splitlines()]
        assert len(records) == 20000
        totals = []
        for index, prefix, encoding in [(7, "F", ">f"), (8, "D", ">d")]:
            tags = [row[index] for row in records if row[index] != "N"]
            assert all(tag.startswith(prefix) for tag in tags)
            values = [struct.unpack(encoding, bytes.fromhex(tag[1:]))[0] for tag in tags]
            totals.append(exact(values))
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        sql = ("SELECT Sum(SingleValue),Sum(DoubleValue),DSum('SingleValue','IndexedRows'),"
               "DSum('DoubleValue','IndexedRows') FROM IndexedRows")
        residuals = []
        for row in records:
            if row[8] == "N":
                continue
            assert row[1].startswith("I32:")
            identifier = int(row[1][4:])
            value = struct.unpack(">d", bytes.fromhex(row[8][1:]))[0]
            residuals.append(value - 1e200 if identifier % 3 == 2 else value)
        expected = exact(residuals)
        expression = "IIf(ID Mod 3=2,DoubleValue-1e200,DoubleValue)"
        for index in [None, "IdAsc", "IdDesc"]:
            configured = options + (["--index", "IndexedRows", index] if index else [])
            query(sql, totals + totals, configured)
            query(f"SELECT Sum({expression}),DSum('{expression}','IndexedRows') FROM IndexedRows",
                  [expected, expected], configured)

            # Raw native Money and IEEE fields, selected dynamically rather than
            # coerced by a UNION schema. Money prefixes here remain in range.
            for field_index, field, encoding in [(7, "SingleValue", ">f"), (8, "DoubleValue", ">d")]:
                values = []
                for row in records:
                    identifier = int(row[1][4:])
                    tag = row[6] if identifier % 2 == 0 else row[field_index]
                    if tag == "N":
                        continue
                    if identifier % 2 == 0:
                        assert tag.startswith("C")
                        values.append(float(Fraction(int(tag[1:]), 10000)))
                    else:
                        values.append(struct.unpack(encoding, bytes.fromhex(tag[1:]))[0])
                mixed = f"IIf(ID Mod 2=0,[Money],{field})"
                query(f"SELECT Sum({mixed}),DSum('{mixed}','IndexedRows') FROM IndexedRows",
                      [exact(values), exact(values)], configured)
        assert path.read_bytes() == before
    print(f"Passed {controls} independent floating-sum Fraction/native-field controls; native Access unrun")


if __name__ == "__main__":
    main()
