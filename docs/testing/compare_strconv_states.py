"""Independent StrConv state/representation controls; native Access unrun."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def text(value):
    return display("T" + value.encode().hex())


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
        for code in [0, 1, 11, 255, 65535]:
            error = f"CVErr({code})"
            for expression, diagnostic in [
                (f"StrConv({error},-1,{error})", "domain"),
                (f"StrConv({error},4,{error})", "not implemented"),
                (f"StrConv({error},0,{error})", "type mismatch"),
                (f"StrConv(NULL,{error})", "type mismatch"),
                (f"StrConv({error},0,0)", "domain"),
                (f"StrConv(CVar({error}),0)", "type mismatch"),
                (f"StrConv({error},0,1/0)", "division by zero"),
            ]:
                query(mode, f"SELECT 1,{expression}", diagnostic=diagnostic)
            source = f"Error {code}"
            for flag, expected in [(1, source.upper()), (2, source.lower()), (3, source)]:
                query(mode, f"SELECT StrConv(CStr({error}),{flag},1033)", [[text(expected)]])
            sql = f"SELECT LenB(StrConv(CStr({error}),128,1033)),StrConv(StrConv(CStr({error}),128,1033),64,1033)"
            query(mode, sql, [[str(len(source.encode("cp1252"))), text(source)]])
        query(mode, "SELECT LenB(StrConv(CVar(MidB('😀',1,1)),0)),VarType(StrConv(ChrB(255),0))", [["1", "8"]])
        query(mode, "SELECT StrConv(ChrB(300),CVErr(11))", diagnostic="overflow")

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
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000", "--execution-steps", "10000000"]
        for index in [[], ["--index", "IndexedRows", "IdAsc"]]:
            chosen = [*options, *index]
            sql = "SELECT StrConv(CStr(CVErr(ID)),2,1033),LenB(StrConv(CStr(CVErr(ID)),128,1033)) FROM IndexedRows WHERE ID=1"
            query("run", sql, [[text("error 1"), "7"]], chosen)
            for expression, diagnostic in [
                ("StrConv(CVar(CVErr(ID)),-1)", "domain"),
                ("StrConv(NULL,CVErr(ID))", "type mismatch"),
                ("StrConv(CVErr(ID),0,0)", "domain"),
                ("StrConv(CVErr(ID),0,1/(ID-1))", "division by zero"),
            ]:
                query("run", f"SELECT 1,{expression} FROM IndexedRows WHERE ID=1", options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} StrConv state controls; native Access unrun.")


if __name__ == "__main__":
    main()
