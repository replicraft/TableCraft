"""Independent character and named numeric formatter state controls.

Only TableCraft executes SQL. Original native identities come from reviewed
read-only getters; exact integer/decimal and character models supply results.
"""

import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256

CHARACTERS = ["Chr", "Chr$", "ChrW"]
FORMATS = ["FormatNumber", "FormatPercent", "FormatCurrency"]
POLICIES = ["--number-format", "en-US", "--character-code-page", "1252"]


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def character(function, code):
    if function == "ChrW":
        text = chr(code)
    else:
        text = bytes([code]).decode("cp1252")
    return display("T" + text.encode().hex())


def formatted(function, code):
    amount = Decimal(code) * (100 if function == "FormatPercent" else 1)
    text = ("$" if function == "FormatCurrency" else "") + format(amount, ".2f")
    if function == "FormatPercent":
        text += "%"
    return display("T" + text.encode().hex())


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
        for function in CHARACTERS:
            query(mode, f"SELECT {function}(NULL)", diagnostic="invalid use of NULL")
            for code in [0, 1, 65, 127, 128, 255, 65535]:
                query(mode, f"SELECT {function}(CVErr({code}))", diagnostic="type mismatch")
                sql = f"SELECT {function}(CLng(CVErr({code})))"
                if code > 255 and function != "ChrW":
                    query(mode, sql, diagnostic="domain")
                else:
                    query(mode, sql, [[character(function, code)]])
        for function in FORMATS:
            for code in [0, 1, 11, 255, 65535]:
                error = f"CVErr({code})"
                for arguments, diagnostic in [
                    (error, "type mismatch"),
                    (f"1,{error}", "type mismatch"),
                    (f"1,2,{error}", "type mismatch"),
                    (f"1,2,-1,{error}", "type mismatch"),
                    (f"1,2,-1,0,{error}", "type mismatch"),
                    (f"{error},256,{error}", "domain"),
                    (f"NULL,{error}", "invalid use of NULL"),
                    (f"{error},1/0", "division by zero"),
                ]:
                    query(mode, f"SELECT {function}({arguments})", diagnostic=diagnostic)
                query(mode, f"SELECT {function}(CLng({error}),2,-1,0,0)", [[formatted(function, code)]])

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
            for function in CHARACTERS:
                query("run", f"SELECT {function}(CLng(CVErr(ID+64))) FROM IndexedRows WHERE ID=1",
                      [[character(function, 65)]], chosen)
                query("run", f"SELECT {function}(CVErr(ID)) FROM IndexedRows WHERE ID=1",
                      options=chosen, diagnostic="type mismatch")
                sql = f"SELECT {function}(IIf(True,CVErr(ID),1/(ID-1))) FROM IndexedRows WHERE ID=1"
                query("run", sql, options=chosen, diagnostic="division by zero")
            for function in FORMATS:
                query("run", f"SELECT {function}(CLng(CVErr(ID)),2,-1,0,0) FROM IndexedRows WHERE ID=1",
                      [[formatted(function, 1)]], chosen)
                for arguments, diagnostic in [
                    ("CVErr(ID)", "type mismatch"),
                    ("CVErr(ID),256,CVErr(ID)", "domain"),
                    ("NULL,CVErr(ID)", "invalid use of NULL"),
                ]:
                    query("run", f"SELECT 1,{function}({arguments}) FROM IndexedRows WHERE ID=1",
                          options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} character/format state controls; native Access unrun.")


if __name__ == "__main__":
    main()
