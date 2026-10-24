"""Independent DATE-serial means; inert Jackcess fields and Fraction arithmetic.

Only TableCraft executes SQL. The Double result profile is not native Access
result-type or rounding qualification.
"""
import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
from pathlib import Path
import random
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def mean(values):
    present = [value for value in values if value is not None]
    return float(sum(present, Fraction()) / len(present)) if present else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(sql, expected, options=()):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, "--format", "tsv", *map(str, options)],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        assert result.returncode == 0 and not result.stderr, (sql, result)
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert len(rows) == len(expected) + 1, (sql, rows, expected)
        for actual, wanted in zip(rows[1:], expected):
            assert len(actual) == len(wanted), (sql, actual, wanted)
            for cell, value in zip(actual, wanted):
                if value is None:
                    assert cell == "NULL", (sql, cell)
                elif isinstance(value, bool):
                    assert cell == ("TRUE" if value else "FALSE"), (sql, cell)
                else:
                    assert struct.pack(">d", float(cell)) == struct.pack(">d", float(value)), (sql, cell, value)
        controls += 1

    cases = [[], [None], [2.25, None, -1.25], [-0.25, 0.25], [-0.0],
             [-657434.0, 2958465.9999999995], [5e-324], [-5e-324, 5e-324]]
    generator = random.Random(20261023)
    for _ in range(64):
        cases.append([None if generator.randrange(5) == 0 else generator.randint(-40000, 40000) / 4
                      for _ in range(generator.randrange(1, 16))])
    for values in cases:
        inputs = " UNION ALL ".join(
            "SELECT NULL AS Moment" if value is None else f"SELECT CDate({value!r}) AS Moment"
            for value in values
        ) or "SELECT CDate(0) AS Moment WHERE False"
        expected = mean([Fraction.from_float(value) if value is not None else None for value in values])
        query(f"SELECT Avg(Moment) FROM ({inputs}) AS T", [[expected]])

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
        groups = {key: [] for key in range(3)}
        mixed = []
        for row in records:
            assert row[1].startswith("I32:")
            identifier = int(row[1][4:])
            tag = row[9]
            value = None
            if tag != "N":
                assert tag.startswith("Y64:")
                serial = struct.unpack(">d", bytes.fromhex(tag[4:]))[0]
                value = Fraction.from_float(serial)
            values.append(value)
            groups[identifier % 3].append(value)
            mixed.append(value if identifier % 2 == 0 else Fraction())
        expected = mean(values)
        headings = [mean(groups[key]) for key in range(3)]
        options = ["--database", path, "--native-bytes", "67108864",
                   "--native-rows", "100000", "--native-steps", "10000000",
                   "--execution-steps", "10000000"]
        for index in [None, "IdAsc", "IdDesc"]:
            configured = options + (["--index", "IndexedRows", index] if index else [])
            query("SELECT Avg(Stamp),DAvg('Stamp','IndexedRows'),Avg(CDbl(Stamp)) "
                  "FROM IndexedRows WHERE Stamp IS NOT NULL", [[expected] * 3], configured)
            query("SELECT D.M FROM (SELECT Avg(Stamp) AS M FROM IndexedRows) AS D",
                  [[expected]], configured)
            query("SELECT ID Mod 3 AS B,Avg(Stamp) AS M FROM IndexedRows GROUP BY ID Mod 3 ORDER BY B",
                  [[key, headings[key]] for key in range(3)], configured)
            query("TRANSFORM Avg(Stamp) SELECT True AS G,Avg(Stamp) AS Total "
                  "FROM IndexedRows GROUP BY True PIVOT ID Mod 3 IN (0,1,2)",
                  [[True, expected, *headings]], configured)
            query("SELECT Avg(Stamp),DAvg('Stamp','IndexedRows','ID=0') FROM IndexedRows WHERE False",
                  [[None, None]], configured)
            query("SELECT Avg(IIf(ID Mod 2=0,Stamp,CDbl(0))) FROM IndexedRows",
                  [[mean(mixed)]], configured)
        assert path.read_bytes() == before

    print(f"Passed {controls} DATE-average Fraction/native-field controls; native Access unrun")


if __name__ == "__main__":
    main()
