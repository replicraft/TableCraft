"""Independent repetition state/control checks; native Access remains unrun."""

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


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


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
            [str(args.cli), "query", mode, sql, *map(str, options)],
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
        for code in [0, 1, 11, 65, 255, 65535]:
            error = f"CVErr({code})"
            query(mode, f"SELECT String(NULL,{error}),String({error},NULL)", [["NULL", "NULL"]])
            query(mode, f"SELECT VarType(String(NULL,{error})),VarType(String(2,CStr({error})))", [["1", "8"]])
            for count in [0, 1, 2, 5]:
                query(mode, f"SELECT String({count},CStr({error}))", [[quoted("E" * count)]])
                query(mode, f"SELECT String({count},{error})", diagnostic="type mismatch")
            for sql, diagnostic in [
                (f"SELECT Space({error})", "type mismatch"),
                (f"SELECT String(-1,{error})", "domain"),
                (f"SELECT String({error},1/0)", "division by zero"),
                (f"SELECT String(NULL,IIf(True,{error},1/0))", "division by zero"),
                (f"SELECT String({error},CStr(NULL))", "invalid use of NULL"),
            ]:
                query(mode, sql, diagnostic=diagnostic)

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
            sql = (
                "SELECT ID,String(NULL,CVErr(ID)),String(CVErr(ID),NULL),"
                "String(ID,CStr(CVErr(ID))) FROM IndexedRows WHERE ID=1"
            )
            query("run", sql, [["1", "NULL", "NULL", quoted("E")]], chosen)
            for expression, diagnostic in [
                ("String(-ID,CVErr(ID))", "domain"),
                ("Space(CVErr(ID))", "type mismatch"),
                ("String(0,CVErr(ID))", "type mismatch"),
                ("String(CVErr(ID),1/(ID-1))", "division by zero"),
            ]:
                query("run", f"SELECT 1,{expression} FROM IndexedRows WHERE ID=1",
                      options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} repetition state controls; native Access unrun.")


if __name__ == "__main__":
    main()
