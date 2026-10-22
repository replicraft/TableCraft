"""Check provisional text/numeric UNION conversion with original Python models."""
import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES as INDEX_HASHES
from compare_numeric_text import floating
from compare_union_types import JAR_SHA256, NATIVE_HASHES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--indexed-directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.native_directory or args.indexed_directory):
        parser.error("Native directories and --jar must be supplied together")
    controls = 0

    def check(body, expected, options=(), accepted=True):
        nonlocal controls
        sql = f"SELECT N,TypeName(N) AS Carrier FROM ({body}) AS U"
        result = subprocess.run([str(args.cli), "query", "run", sql, *map(str, options)],
                                capture_output=True, text=True, encoding="utf-8")
        assert result.returncode == (0 if accepted else 2), (body, result.stderr)
        if accepted:
            assert not result.stderr, result.stderr
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[0] == ["N", "Carrier"]
            actual = []
            for value, carrier in rows[1:]:
                assert json.loads(carrier) == ("Null" if value == "NULL" else "String"), (body, carrier)
                actual.append(None if value == "NULL" else json.loads(value))
            assert actual == expected, (body, actual, expected)
        else:
            assert not result.stdout, (body, result.stdout[:100])
        controls += 1

    cases = [("CBool(1)", "True"), ("CBool(0)", "False"), ("CByte(255)", "255"),
             ("CInt(-32768)", "-32768"), ("CLng(2147483647)", "2147483647"),
             ("CCur(1.25)", "1.25"), ("CDec(1.25)", "1.25"),
             ("CDec('79228162514264337593543950335')", "79228162514264337593543950335")]
    for value in [-9223372036854775808, -9007199254740993, -1, 0, 1, 9007199254740993, 9223372036854775807]:
        cases.append((f"CLngLng({value})", str(value)))
    for value in [-0.0, 0.1, 1.25, 1.0e20]:
        single = struct.unpack("f", struct.pack("f", value))[0]
        cases.append((f"CSng({value!r})", floating(single, 7).lstrip(" ")))
        cases.append((f"CDbl({value!r})", floating(value, 15).lstrip(" ")))
    for profile in ["invariant", "de-de"]:
        options = ["--number-format", profile, "--collation", "ordinal"]
        for expression, spelling in cases:
            expected = spelling.replace(".", ",") if profile == "de-de" else spelling
            for reverse in [False, True]:
                arms = [expression, "'tail雪🦀'"]
                values = [expected, "tail雪🦀"]
                if reverse:
                    arms.reverse()
                    values.reverse()
                check(f"SELECT {arms[0]} AS N UNION ALL SELECT {arms[1]}", values, options)
            literal = expected.replace("'", "''")
            for all_rows in [False, True]:
                check(f"SELECT {expression} AS N UNION {'ALL' if all_rows else ''} SELECT '{literal}'",
                      [expected] * (2 if all_rows else 1), options)
        check("SELECT 9007199254740993 AS N UNION ALL SELECT CDbl(0) UNION ALL SELECT 'tail'",
              ["9007199254740993", "0", "tail"], options)
        check("SELECT 1 AS N UNION ALL SELECT 'tail' WHERE False", ["1"], options)
        check("SELECT Null AS N UNION ALL SELECT CInt(1) UNION ALL SELECT 'tail'", [None, "1", "tail"], options)
        check("SELECT 1 AS N UNION ALL SELECT 'tail' UNION ALL SELECT 1/0", None, options, accepted=False)
    check("SELECT CInt(1) AS N WHERE False UNION ALL SELECT 'tail'", ["tail"])
    check("SELECT True AS N UNION ALL SELECT 'tail'", ["True", "tail"])
    check("SELECT 1 AS N UNION ALL SELECT 'tail'", None, accepted=False)
    check("SELECT 1 AS N UNION SELECT '1'", None, ["--number-format", "invariant"], accepted=False)

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    if args.native_directory:
        for suffix, digest in NATIVE_HASHES.items():
            path = args.native_directory / ("unicode-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "AData"], text=True, encoding="utf-8")
            identifiers = [line.split("\t")[1].removeprefix("I32:") for line in output.splitlines()]
            for reverse in [False, True]:
                body = "SELECT ID AS N FROM AData UNION ALL SELECT 'tail'"
                expected = [*identifiers, "tail"]
                if reverse:
                    body = "SELECT 'tail' AS N UNION ALL SELECT ID FROM AData"
                    expected = ["tail", *identifiers]
                check(body, expected, ["--database", path, "--number-format", "invariant"])
            assert path.read_bytes() == original
    if args.indexed_directory:
        for suffix, digest in INDEX_HASHES.items():
            path = args.indexed_directory / ("index-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            for index in ["IdAsc", "IdDesc"]:
                output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                    str(RECIPES / "IndexRowsOracle.java"), str(path), "IndexedRows", index, "ID"], text=True, encoding="utf-8")
                identifiers = [line for line in output.splitlines() if int(line) >= 19998]
                check("SELECT ID AS N FROM IndexedRows WHERE ID>=19998 UNION ALL SELECT 'tail'",
                      [*identifiers, "tail"], ["--database", path, "--index", "IndexedRows", index, "--number-format", "invariant"])
            # The native Short Text member supplies its stored General identity;
            # numeric output is converted but the required field policy survives.
            check("SELECT ID AS N FROM IndexedRows WHERE ID>=19998 UNION SELECT Label FROM IndexedRows WHERE ID>=19998",
                  ["19998", "19999", "20000", "a", "b", None],
                  ["--database", path, "--index", "IndexedRows", "IdAsc", "--number-format", "invariant", "--collation", "database"])
            assert path.read_bytes() == original
    print(f"Passed {controls} independent text/numeric UNION value/type/format/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
