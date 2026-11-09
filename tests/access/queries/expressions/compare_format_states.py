"""Independent Format state/control models; native Access remains unrun."""

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
            [str(args.cli), "query", mode, sql, "--number-format", "en-US", "--case-locale", "und", *map(str, options)],
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
            for arguments, diagnostic in [
                (error, "type mismatch"),
                (f"{error},NULL", "invalid use of NULL"),
                (f"NULL,{error}", "type mismatch"),
                (f"{error},{error},8", "domain"),
                (f"{error},{error},8,NULL", "invalid use of NULL"),
                (f"{error},'0',{error}", "type mismatch"),
                (f"{error},'0',8,{error}", "domain"),
                (f"{error},'0',1,{error}", "type mismatch"),
                (f"{error},'0',8,1/0", "division by zero"),
            ]:
                query(mode, f"SELECT 1,Format({arguments})", diagnostic=diagnostic)
            query(mode, f"SELECT Format(CLng({error}),'0.00')", [[text(f"{code}.00")]])
            query(mode, f"SELECT Format(CStr({error}),'@')", [[text(f"Error {code}")]])
            query(mode, f"SELECT Format(CStr({error}),'>')", [[text(f"ERROR {code}")]])
            query(mode, f"SELECT VarType(Format(CLng({error}),'0')),VarType(Format(CStr({error}),'@'))", [["8", "8"]])

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
            sql = "SELECT Format(CLng(CVErr(ID)),'0.00'),Format(CStr(CVErr(ID)),'@') FROM IndexedRows WHERE ID=1"
            query("run", sql, [[text("1.00"), text("Error 1")]], chosen)
            for arguments, diagnostic in [
                ("CVErr(ID)", "type mismatch"),
                ("CVErr(ID),'0',8", "domain"),
                ("CVErr(ID),CVErr(ID),8,NULL", "invalid use of NULL"),
                ("CVErr(ID),NULL", "invalid use of NULL"),
                ("1,'0',CVErr(ID)", "type mismatch"),
                ("CVErr(ID),'0',1/(ID-1)", "division by zero"),
            ]:
                query("run", f"SELECT 1,Format({arguments}) FROM IndexedRows WHERE ID=1", options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} Format state controls; native Access unrun.")


if __name__ == "__main__":
    main()
