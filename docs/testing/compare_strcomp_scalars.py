"""Independent scalar spellings and UTF-16 StrComp models; no Access SQL oracle."""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_strcomp import binary, quoted
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected, options=(), diagnostic=None):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    for profile in ["en-us", "de-de"]:
        cases = [("True", "True"), ("False", "False")]
        for value in [-(2**63), -(2**53+1), 0, 2**53+1, 2**63-1]:
            cases.append((f"CLngLng({value})", str(value)))
        for quarter in range(-17, 18):
            value = Decimal(quarter) / 4
            invariant = str(value)
            spelling = invariant.replace(".", ",") if profile == "de-de" else invariant
            for constructor in ["CSng", "CDbl", "CCur", "CDec"]:
                cases.append((f"{constructor}({invariant})", spelling))
        cases.append(("#1900-01-01#", "1900-01-01"))
        for mode in ["eval", "run"]:
            options = ["--number-format", profile, "--date-format", "iso"]
            for expression, spelling in cases:
                text = quoted(spelling)
                sql = (f"SELECT StrComp({expression},{text},0),StrComp({text},{expression},0),"
                       f"VarType(StrComp({expression},{text},0))")
                query(mode, sql, [["0", "0", "2"]], options)
            query(mode, "SELECT StrComp(10,2,0)", [[str(binary("10", "2"))]], options)
            query(mode, "SELECT StrComp(1,Null,0)", [["NULL"]])
            query(mode, "SELECT StrComp(1,2,3)", None, diagnostic="invalid arithmetic domain")
            query(mode, "SELECT StrComp(Null,1/0,0)", None, options, diagnostic="division by zero")

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        ids = [int(tag[4:]) for tag in tags]
        selected = [value for value in ids if binary(str(value), "20") < 0]
        expected = [[str(len(selected)), str(sum(selected))]]
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000", "--number-format", "en-us"]
        sql = "SELECT Count(*),Sum(ID) FROM IndexedRows WHERE StrComp(ID,20,0)<0"
        for index in [None, "IdAsc", "IdDesc"]:
            query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent StrComp scalar controls; native Access unrun.")


if __name__ == "__main__":
    main()
