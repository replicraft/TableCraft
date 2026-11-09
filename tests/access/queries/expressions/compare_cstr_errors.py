"""Check explicit CStr Error/Empty conversion with original standard-code models."""
import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import (
    INPUT_HASHES as INDEX_HASHES,
    registered_input_hash,
)
from compare_union_types import JAR_SHA256, NATIVE_HASHES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--indexed-directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.native_directory or args.indexed_directory):
        parser.error("Native directories and --jar must be supplied together")
    controls = 0

    def query(mode, sql, expected, options=(), accepted=True):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                capture_output=True, text=True, encoding="utf-8")
        assert result.returncode == (0 if accepted else 2), (mode, sql, result.stderr)
        if accepted:
            assert not result.stderr
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]
            normalized = [[json.loads(value) if value.startswith('"') else value for value in row] for row in rows]
            assert normalized == expected, (mode, sql, normalized, expected)
            controls += sum(map(len, expected))
        else:
            assert not result.stdout, (mode, sql, result.stdout[:100])
            controls += 1

    rng = random.Random(0x43535452455252)
    codes = [0, 1, 9, 10, 99, 100, 999, 1000, 2001, 32767, 32768, 65535]
    codes += [rng.randrange(65536) for _ in range(128)]
    for mode in ("eval", "run"):
        for start in range(0, len(codes), 8):
            expressions, expected = [], []
            for code in codes[start:start + 8]:
                expressions.extend([f"CStr(CVErr({code}))", f"CStr(CVar(CVErr({code})))",
                                    f"CStr(IIf(True,CVErr({code}),0))", f"IsError(CStr(CVErr({code})))"])
                expected.extend([f"Error {code}"] * 3 + ["FALSE"])
            query(mode, "SELECT " + ",".join(expressions), [expected])
        for profile in ("invariant", "de-de"):
            query(mode, "SELECT CStr(CVErr(65535)),CStr(Nz(Null)),TypeName(CStr(CVErr(1))),VarType(CStr(CVErr(1)))",
                  [["Error 65535", "", "String", "8"]], ["--number-format", profile])
        for sql in ["SELECT CStr(Null)", "SELECT CStr(CVErr(-1))", "SELECT CStr(CVErr(65536))",
                    "SELECT CStr(1/0)", "SELECT CStr(IIf(True,CVErr(1),1/0))", "SELECT CVErr(1) & 'x'",
                    "SELECT CStr(1)", "SELECT CStr(1,1/0)", "SELECT CStr(CVErr(1)),1/0"]:
            query(mode, sql, None, accepted=False)
    query("run", "SELECT E,CStr(E) FROM (SELECT CVErr(2001) AS E) AS Q", [["#ERROR(2001)", "Error 2001"]])
    query("run", "SELECT CStr(First(E)),CStr(Last(E)) FROM (SELECT CVErr(1) AS E UNION ALL SELECT CVErr(2)) AS Q",
          [["Error 1", "Error 2"]])
    query("run", "SELECT CStr(E) FROM (SELECT Nz(Null) AS E) AS Q", [[""]])

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    if args.native_directory:
        for suffix, digest in NATIVE_HASHES.items():
            path = args.native_directory / ("unicode-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            rows = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "AData"], text=True, encoding="utf-8")
            codes = [int(line.split("\t")[1].removeprefix("I32:")) for line in rows.splitlines()]
            query("run", "SELECT CStr(CVErr(ID)) FROM AData", [[f"Error {code}"] for code in codes], ["--database", path])
            assert path.read_bytes() == original
    if args.indexed_directory:
        for suffix, digest in INDEX_HASHES.items():
            path = args.indexed_directory / ("index-" + suffix)
            digest = registered_input_hash(path, digest)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            for index in ("IdAsc", "IdDesc"):
                rows = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                    str(RECIPES / "IndexRowsOracle.java"), str(path), "IndexedRows", index, "ID"], text=True, encoding="utf-8")
                codes = [int(line) for line in rows.splitlines() if int(line) >= 19998]
                options = ["--database", path, "--index", "IndexedRows", index]
                query("run", "SELECT CStr(CVErr(ID)) FROM IndexedRows WHERE ID>=19998", [[f"Error {code}"] for code in codes], options)
                query("run", "SELECT CStr(First(CVErr(ID))),CStr(Last(CVErr(ID))) FROM IndexedRows WHERE ID>=19998",
                      [[f"Error {codes[0]}", f"Error {codes[-1]}"]], options)
            assert path.read_bytes() == original
    print(f"Passed {controls} CStr Error/Empty/model/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
