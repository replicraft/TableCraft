"""Independent CStr spellings, UTF-16 lengths and byte operations, without Access SQL."""
import argparse
import csv
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_replacement import spelling as display_text
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(mode, sql, expected, options=()):
        nonlocal controls
        process = subprocess.run([str(args.cli), "query", mode, sql, *map(str, options)],
                                 capture_output=True, text=True, encoding="utf-8", check=False)
        if expected is None:
            assert process.returncode == 2 and not process.stdout, (sql, process)
        else:
            assert process.returncode == 0 and not process.stderr, (sql, process)
            rows = list(csv.reader(StringIO(process.stdout), delimiter="\t", strict=True))
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
            for cast in ["CSng", "CDbl", "CCur", "CDec"]:
                cases.append((f"{cast}({invariant})", spelling))
        cases.append(("#1900-01-01#", "1900-01-01"))
        for mode in ["eval", "run"]:
            options = ["--number-format", profile, "--date-format", "iso"]
            for expression, spelling in cases:
                data = spelling.encode("utf-16le")
                sql = (f"SELECT LeftB({expression},2),RightB({expression},2),MidB({expression},1,2),"
                       f"AscB({expression}),InStrB(1,{expression},5,0),StrConv({expression},0),"
                       f"LenB(StrConv({expression},128,1033)),Len({expression}),LenB({expression}),"
                       f"Len(CVar({expression})),LenB(CVar({expression}))")
                expected = [[display_text(data[:2].decode("utf-16le")), display_text(data[-2:].decode("utf-16le")),
                             display_text(data[:2].decode("utf-16le")), str(data[0]),
                             str(data.find("5".encode("utf-16le")) + 1), display_text(spelling),
                             str(len(spelling.encode("ascii"))), str(len(data) // 2), str(len(data)),
                             str(len(data) // 2), str(len(data))]]
                query(mode, sql, expected, options)
            for sql in ["SELECT LeftB(123,2)", "SELECT RightB(123,2)",
                        "SELECT MidB(123,1)", "SELECT AscB(123)",
                        "SELECT InStrB(1,123,2,0)", "SELECT StrConv(123,0)",
                        "SELECT Len(123)", "SELECT LenB(123)"]:
                query(mode, sql, None)
            for sql in ["SELECT 1,LeftB(123,1)", "SELECT StrConv(123,1,1/0)"]:
                query(mode, sql, None, options)

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        original = path.read_bytes()
        assert hashlib.sha256(original).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
        tags = [line.split("\t")[1] for line in output.splitlines()]
        assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
        identifiers = {8, 9, 10, 99, 100, *range(12340, 12345)}
        selected = sorted(int(tag[4:]) for tag in tags if int(tag[4:]) in identifiers)
        assert len(selected) == 10
        expected = []
        for identifier in selected:
            text = str(identifier)
            raw = text.encode("utf-16le")
            expected.append([text, display_text(text[0]), display_text(text[-1]), display_text(text[1:2]), str(raw[0]),
                             str(raw.find("34".encode("utf-16le")) + 1), display_text(text),
                             str(len(raw) // 2), str(len(raw))])
        sql = ("SELECT ID,LeftB(ID,2),RightB(ID,2),MidB(ID,3,2),AscB(ID),"
               "InStrB(1,ID,34,0),StrConv(ID,0),Len(ID),LenB(ID) FROM IndexedRows "
               "WHERE ID IN (8,9,10,99,100,12340,12341,12342,12343,12344) ORDER BY ID")
        options = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000", "--number-format", "en-us"]
        for index in [None, "IdAsc", "IdDesc"]:
            query("run", sql, expected, [*options, *(["--index", "IndexedRows", index] if index else [])])
        assert path.read_bytes() == original
    print(f"Passed {controls} independent length/byte scalar-source controls; native Access unrun")


if __name__ == "__main__":
    main()
