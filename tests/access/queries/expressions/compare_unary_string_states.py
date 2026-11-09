"""Independent unary string state and native getter integration controls.

Only TableCraft executes SQL. Expected ASCII results come from Python models;
the reviewed read-only reference reader supplies original native identities.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_error_messages import quoted
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256

FUNCTIONS = ["Len", "LTrim", "RTrim", "Trim", "LCase", "UCase", "Asc", "AscW", "StrReverse", "Val"]
NULL_REFUSALS = {"Asc", "AscW", "StrReverse", "Val"}
EMPTY_REFUSALS = {"Asc", "AscW"}
TYPES = {"Len": "3", "Asc": "2", "AscW": "2", "Val": "5"}
POLICIES = ["--case-locale", "und"]


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def model(function, source):
    if function == "Len":
        return str(len(source.encode("utf-16-le")) // 2)
    if function in {"Asc", "AscW"}:
        return str(ord(source[0]))
    if function == "Val":
        # These deliberately nonnumeric English Error spellings have no prefix.
        assert not source or source.startswith("Error ")
        return "0"
    operations = {
        "LTrim": lambda text: text.lstrip(" "),
        "RTrim": lambda text: text.rstrip(" "),
        "Trim": lambda text: text.strip(" "),
        "LCase": str.lower,
        "UCase": str.upper,
        "StrReverse": lambda text: text[::-1],
    }
    return quoted(operations[function](source))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    controls = 0

    def query(mode, sql, expected=None, options=(), diagnostic=None):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *POLICIES, *map(str, options)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, "non-atomic refusal")
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows[1:], expected)
        controls += 1

    for mode in ["eval", "run"]:
        for function in FUNCTIONS:
            if function in NULL_REFUSALS:
                query(mode, f"SELECT {function}(NULL)", diagnostic="invalid use of NULL")
            else:
                query(mode, f"SELECT {function}(NULL)", [["NULL"]])
            if function in EMPTY_REFUSALS:
                query(mode, f"SELECT {function}(Nz(NULL))", diagnostic="domain")
            else:
                query(mode, f"SELECT {function}(Nz(NULL))", [[model(function, "")]])
            query(mode, f"SELECT VarType({function}(CStr(CVErr(11))))", [[TYPES.get(function, "8")]])
            for code in [0, 1, 11, 255, 65535]:
                query(mode, f"SELECT {function}(CVErr({code}))", diagnostic="type mismatch")
                query(mode, f"SELECT {function}(IIf(True,CVErr({code}),1/0))", diagnostic="division by zero")
                query(mode, f"SELECT {function}(CStr(CVErr({code})))", [[model(function, f"Error {code}")]])

    for suffix, original_digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = registered_input_hash(path, original_digest)
        assert digest(path) == before
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in reference.splitlines()]
        assert len(identities) == len(set(identities)) == 20_000 and 1 in identities
        options = [
            "--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
            "--native-steps", "10000000", "--execution-steps", "10000000",
        ]
        for index in [[], ["--index", "IndexedRows", "IdAsc"]]:
            chosen = [*options, *index]
            for function in FUNCTIONS:
                sql = f"SELECT {function}(CStr(CVErr(ID))) FROM IndexedRows WHERE ID=1"
                query("run", sql, [[model(function, "Error 1")]], chosen)
                query("run", f"SELECT 1,{function}(CVErr(ID)) FROM IndexedRows WHERE ID=1",
                      options=chosen, diagnostic="type mismatch")
                sql = f"SELECT 1,{function}(IIf(True,CVErr(ID),1/(ID-1))) FROM IndexedRows WHERE ID=1"
                query("run", sql, options=chosen, diagnostic="division by zero")
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} unary string state controls; native Access unrun.")


if __name__ == "__main__":
    main()
