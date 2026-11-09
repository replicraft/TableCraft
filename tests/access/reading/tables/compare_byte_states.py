"""Independent expression-state and UTF-16LE byte models; Access unrun."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
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
        for code in [0, 1, 11, 255, 65535]:
            error = f"CVErr({code})"
            raw = f"Error {code}".encode("utf-16-le")
            for function in ["LenB", "AscB", "ChrB"]:
                query(mode, f"SELECT 1,{function}({error})", diagnostic="type mismatch")
            for function in ["LeftB", "RightB", "MidB"]:
                query(mode, f"SELECT 1,{function}({error},-1)", diagnostic="domain")
                for count in [0, 1, 3, 7]:
                    suffix = f",1,{count}" if function == "MidB" else f",{count}"
                    query(mode, f"SELECT LenB({function}(CStr({error}){suffix}))", [[str(min(len(raw), count))]])
            for expression, diagnostic in [
                (f"MidB({error},0,{error})", "type mismatch"),
                (f"LeftB(NULL,{error})", "type mismatch"),
                (f"InStrB({error},NULL,{error},3)", "not implemented"),
                (f"InStrB(1,123,{error},0)", "number-format"),
                (f"LeftB({error},1/0)", "division by zero"),
            ]:
                query(mode, f"SELECT 1,{expression}", diagnostic=diagnostic)
            query(mode, f"SELECT InStrB(1,NULL,{error},0)", [["NULL"]])
            sql = f"SELECT AscB(ChrB(CLng({error})))"
            if code > 255:
                query(mode, sql, diagnostic="overflow")
            else:
                query(mode, sql, [[str(code)]])
            query(mode, f"SELECT LenB(CStr({error})),AscB(CStr({error}))", [[str(len(raw)), "69"]])
            for needle in ["r", str(code), "", "z"]:
                position = raw.find(needle.encode("utf-16-le")) + 1
                query(mode, f"SELECT InStrB(1,CStr({error}),'{needle}',0)", [[str(position)]])

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
            sql = "SELECT LenB(LeftB(CStr(CVErr(ID)),3)),AscB(ChrB(CLng(CVErr(ID+64)))) FROM IndexedRows WHERE ID=1"
            query("run", sql, [["3", "65"]], chosen)
            query("run", "SELECT InStrB(1,NULL,CVErr(ID),0) FROM IndexedRows WHERE ID=1", [["NULL"]], chosen)
            for expression, diagnostic in [
                ("LeftB(CVErr(ID),-1)", "domain"),
                ("MidB(CVErr(ID),0,CVErr(ID))", "type mismatch"),
                ("LenB(CVErr(ID))", "type mismatch"),
                ("InStrB(CVErr(ID),NULL,CVErr(ID),3)", "not implemented"),
                ("InStrB(1,ID,CVErr(ID),0)", "number-format"),
                ("RightB(CVErr(ID),1/(ID-1))", "division by zero"),
            ]:
                query("run", f"SELECT 1,{expression} FROM IndexedRows WHERE ID=1", options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} byte-state controls; native Access unrun.")


if __name__ == "__main__":
    main()
