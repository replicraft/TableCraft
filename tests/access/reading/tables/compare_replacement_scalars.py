"""Independent CStr spelling and UTF-16 Replace controls; no Access SQL oracle."""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_replacement import model, spelling
from compare_strcomp import quoted
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    if args.jar:
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
            text = invariant.replace(".", ",") if profile == "de-de" else invariant
            for constructor in ["CSng", "CDbl", "CCur", "CDec"]:
                cases.append((f"{constructor}({invariant})", text))
        cases.append(("#1900-01-01#", "1900-01-01"))
        for mode in ["eval", "run"]:
            options = ["--number-format", profile, "--date-format", "iso"]
            for expression, text in cases:
                wrapped = "🦀" + text + text + "🦀"
                sql = (f"SELECT Replace({expression},'2','X'),"
                       f"Replace({quoted(wrapped)},{expression},'X',3,1),"
                       f"Replace('aaa','a',{expression},2,1)")
                expected = [[spelling(model(text, "2", "X", 1, -1)),
                             spelling(model(wrapped, text, "X", 3, 1)),
                             spelling(model("aaa", "a", text, 2, 1))]]
                query(mode, sql, expected, options)
            for expression in ["Replace(NULL,1,2)", "Replace('','x',NULL)"]:
                query(mode, "SELECT " + expression, None, diagnostic="invalid use of NULL in conversion")
            query(mode, "SELECT Replace(123,23,45,0)", None, diagnostic="invalid arithmetic domain")
            query(mode, "SELECT Replace('a','a',2,1,0)", None, diagnostic="explicit number-format policy")
            query(mode, "SELECT Replace(NULL,'a',1/0)", None, diagnostic="division by zero")

    if args.directory:
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
            expected = [[str(value), spelling(model(str(value), "2", "X", 1, -1))] for value in sorted(ids)[:5]]
            options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                       "--native-steps", "10000000", "--execution-steps", "10000000", "--number-format", "en-us"]
            sql = "SELECT TOP 5 ID,Replace(ID,2,'X') FROM IndexedRows ORDER BY ID"
            for index in [None, "IdAsc", "IdDesc"]:
                query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
            assert path.read_bytes() == before
    print(f"Passed {controls} independent scalar replacement controls; native Access unrun.")


if __name__ == "__main__":
    main()
