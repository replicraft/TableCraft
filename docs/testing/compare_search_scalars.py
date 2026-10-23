"""Independent scalar spellings and UTF-16 substring models; no Access SQL oracle."""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_strcomp import quoted
from compare_union_types import JAR_SHA256


def position(haystack, needle, reverse=False):
    haystack = haystack.encode("utf-16-le")
    needle = needle.encode("utf-16-le")
    matches = [i // 2 + 1 for i in range(0, len(haystack) - len(needle) + 1, 2)
               if haystack[i:i + len(needle)] == needle]
    return (matches[-1] if reverse else matches[0]) if matches else 0


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
                                text=True, encoding="utf-8", capture_output=True, check=False)
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
        for quarter in range(-8, 9):
            value = Decimal(quarter) / 4
            invariant = str(value)
            spelling = invariant.replace(".", ",") if profile == "de-de" else invariant
            for constructor in ["CSng", "CDbl", "CCur", "CDec"]:
                cases.append((f"{constructor}({invariant})", spelling))
        cases.append(("#1900-01-01#", "1900-01-01"))
        for mode in ["eval", "run"]:
            options = ["--number-format", profile, "--date-format", "iso"]
            for expression, spelling in cases:
                wrapped = "🦀" + spelling + "🦀"
                sql = (f"SELECT InStr(1,{expression},'2',0),InStrRev({expression},'2'),"
                       f"InStr(1,{quoted(wrapped)},{expression},0),InStrRev({quoted(wrapped)},{expression}),"
                       f"VarType(InStrRev({expression},'2'))")
                query(mode, sql, [[str(position(spelling, "2")), str(position(spelling, "2", True)), "3", "3", "3"]], options)
            query(mode, "SELECT InStrRev(123,Null)", [["NULL"]])
            query(mode, "SELECT InStr(0,123,Null,0)", None, diagnostic="invalid arithmetic domain")
            query(mode, "SELECT InStrRev(123,Null,1,3)", None, diagnostic="invalid arithmetic domain")
            query(mode, "SELECT InStrRev(123,2)", None, diagnostic="explicit number-format policy")
            query(mode, "SELECT InStrRev(Null,1/0)", None, options, diagnostic="division by zero")

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        ids = [int(tag[4:]) for tag in tags]
        selected = [value for value in ids if position(str(value), "20") > 0]
        expected = [[str(len(selected)), str(sum(selected))]]
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000", "--number-format", "en-us"]
        sql = "SELECT Count(*),Sum(ID) FROM IndexedRows WHERE InStr(1,ID,20,0)>0"
        for index in [None, "IdAsc", "IdDesc"]:
            query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent scalar substring controls; native Access unrun.")


if __name__ == "__main__":
    main()
