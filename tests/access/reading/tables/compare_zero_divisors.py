"""Independent operator-specific zero-divisor matrix and inert native getters.

Only TableCraft executes SQL. Language-reference error classes and raw Jackcess
values are separate evidence from native Access query/error qualification.
"""
import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, error, options=()):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *map(str, options)],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        if error is None:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == [["NULL", "NULL"]], (sql, rows)
        else:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert error in result.stderr, (sql, result.stderr, error)
            other = "division by zero" if error == "numeric overflow" else "numeric overflow"
            assert other not in result.stderr, (sql, result.stderr, other)
        controls += 1

    zeros = ["False", "CByte(0)", "CInt(0)", "CLng(0)", "CLngLng(0)",
             "CSng(0)", "CDbl(-0.0)", "CCur(0)", "CVar(CDate(0))", "'0'", "CDec(0)"]
    for mode, declared in [("eval", False), ("run", False), ("run", True)]:
        options = ["--number-format", "en-us"]
        if declared:
            options.append("--declared-arithmetic")
        for left in zeros:
            for right in zeros:
                error = "division by zero" if "CDec" in (left + right) else "numeric overflow"
                query(mode, f"SELECT 42,{left}/{right}", error, options)
            query(mode, f"SELECT 42,1/{left}", "division by zero", options)
            for operator in ["\\", "Mod"]:
                query(mode, f"SELECT 42,{left} {operator} CDbl(0.5)", "division by zero", options)
        for expression, error in [
            ("CSng(1.401298464324817e-45)/CSng(0)", "division by zero"),
            ("CDbl(4.9406564584124654e-324)/CDbl(-0.0)", "division by zero"),
            ("CCur(0.0001)/0", "division by zero"),
            ("NULL+(0/0)", "numeric overflow"),
            ("IIf(True,1,0/0)", "numeric overflow"),
            ("IsError(0/0)", "numeric overflow"),
            ("0/(1/0)", "division by zero"),
            ("(0/0)/(1/0)", "numeric overflow"),
        ]:
            query(mode, "SELECT 42," + expression, error, options)
        query(mode, "SELECT NULL/0,0/NULL", None, options)

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        selected = [int(tag[4:]) for tag in tags if int(tag[4:]) == 12345]
        assert selected == [12345]
        identifier = selected[0]
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        for index in [None, "IdAsc", "IdDesc"]:
            indexed = options + (["--index", "IndexedRows", index] if index else [])
            for declared in [False, True]:
                configured = indexed + (["--declared-arithmetic"] if declared else [])
                for expression, error in [
                    ("(ID-ID)/(ID-ID)", "numeric overflow"),
                    ("ID/(ID-ID)", "division by zero"),
                    ("CDec(ID-ID)/(ID-ID)", "division by zero"),
                    ("(ID-ID) Mod (ID-ID)", "division by zero"),
                    ("(ID-ID)\\(ID-ID)", "division by zero"),
                ]:
                    sql = f"SELECT 42,{expression} FROM IndexedRows WHERE ID={identifier}"
                    query("run", sql, error, configured)
        assert path.read_bytes() == before
    print(f"Passed {controls} independent zero-divisor/error-class/native-field controls; native Access unrun")


if __name__ == "__main__":
    main()
