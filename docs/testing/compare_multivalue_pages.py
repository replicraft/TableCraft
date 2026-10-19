"""Compare mixed parent pages with independent inert scalar/collection APIs.

Compile original local oracle sources against the reviewed Jackcess artifact.
Only stored data is read; lookup/formula evaluation stays disabled. CLI NaN
presentation intentionally omits payload bits; native APIs retain those bits.
"""
import argparse
import hashlib
import math
import os
from pathlib import Path
import struct
import subprocess
import tempfile

from compare_browsing import display, escaped, run
from compare_complex_links import column_layout
from compare_composite_constraints import roots

RECIPES = Path(__file__).parent


def split_values(cell):
    assert cell.startswith("[") and cell.endswith("]")
    text = cell[1:-1]
    if not text:
        return []
    result, start, quoted, escape = [], 0, False, False
    for index, char in enumerate(text):
        if escape:
            escape = False
        elif char == "\\":
            escape = True
        elif char == '"':
            quoted = not quoted
        elif char == "," and not quoted:
            result.append(text[start:index].strip())
            start = index + 1
    assert not quoted and not escape
    result.append(text[start:].strip())
    return result


def same_value(actual, tag):
    if tag.startswith(("F", "D")):
        code = ">f" if tag[0] == "F" else ">d"
        expected = struct.unpack(code, bytes.fromhex(tag[1:]))[0]
        if math.isnan(expected):
            assert math.isnan(float(actual))
        else:
            assert struct.pack(code, float(actual)).hex() == tag[1:]
    else:
        assert actual == display(tag), (actual, tag)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "matrix2016", "matrix2019", "mixed2016", "mixed2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--matrix2007", type=Path)
    args = parser.parse_args()
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-multivalue-oracle-") as directory:
        run("javac", "-cp", args.jar, "-d", directory, RECIPES / "MultivalueOracle.java", RECIPES / "MultivaluePageOracle.java")
        classpath = str(args.jar) + os.pathsep + directory
        cases = [(args.matrix2016, "Multivalues", False, 5), (args.matrix2019, "Multivalues", False, 6),
                 (args.mixed2016, "MixedValues", True, 5), (args.mixed2019, "MixedValues", True, 6)]
        if args.matrix2007:
            cases.append((args.matrix2007, "Multivalues", False, 2))
        for path, table, cache, code in cases:
            original = path.read_bytes()
            assert original[20:24] == bytes([code, 0, 0, 0])
            oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", classpath, "MultivaluePageOracle", path, table)
            columns, rows = [], {}
            for line in oracle.splitlines():
                cells = line.split("\t")
                if cells[0] == "H":
                    columns.append((cells[1], cells[2]))
                else:
                    rows.setdefault(int(cells[0]), {})[cells[1]] = cells[2:]
            header = "\t".join(escaped(name) + {"S": "", "C": " [cached]", "M": " [multivalue]"}[kind] for name, kind in columns)
            options = ["--multivalues", "--format", "tsv"] + (["--stored-calculations"] if cache else [])
            for offset, limit in [(0, 100), (0, 1), (1, 1), (2, 2), (len(rows), 1), (4_294_967_295, 1)]:
                actual = run(args.cli, "rows", path, table, *options, "--offset", offset, "--limit", limit).splitlines()
                general_options = ["--collections" if option == "--multivalues" else option for option in options]
                general = run(args.cli, "rows", path, table, *general_options, "--offset", offset, "--limit", limit).splitlines()
                assert general == actual
                controls += 1
                assert actual[0] == header
                selected = list(range(len(rows)))[offset:offset + limit]
                assert len(actual) == len(selected) + 1
                for actual_row, ordinal in zip(actual[1:], selected):
                    fields = actual_row.split("\t")
                    assert len(fields) == len(columns)
                    for cell, (name, _) in zip(fields, columns):
                        expected = rows[ordinal][name]
                        if expected[0] == "S":
                            same_value(cell, expected[1])
                        elif expected[1] == "N":
                            assert cell == "NULL" and len(expected) == 2
                        else:
                            values = split_values(cell)
                            assert len(values) == len(expected) - 2
                            for value, tag in zip(values, expected[2:]):
                                same_value(value, tag)
                controls += 1
            refusals = [["rows", path, table], ["query", "run", "SELECT * FROM " + table, "--database", path]]
            if cache:
                refusals += [["rows", path, table, "--multivalues"], ["rows", path, table, "--stored-calculations"]]
                empty = run(args.cli, "rows", path, "EmptyMixedValues", *options)
                assert empty == header + "\n"
                assert run(args.cli, "rows", path, "EmptyMixedValues", *general_options) == empty
                controls += 1
                assert rows[0]["Derived"] == ["S", "I32:41"], "Deliberately stale cache missing"
                controls += 1
            else:
                refusals += [["rows", path, "Overflow", "--multivalues", "--offset", "4294967295"]]
            for command in refusals:
                result = subprocess.run(list(map(str, [args.cli, *command])), capture_output=True)
                assert result.returncode == 2 and not result.stdout
                if "--multivalues" in command:
                    general_command = ["--collections" if option == "--multivalues" else option for option in command]
                    general = subprocess.run(list(map(str, [args.cli, *general_command])), capture_output=True)
                    assert general.returncode == 2 and not general.stdout
                    controls += 1
                controls += 1
            pretty = run(args.cli, "rows", path, table, "--multivalues", "--format", "table", "--limit", 1,
                         *(["--stored-calculations"] if cache else []))
            assert pretty.startswith("┌") and "[multivalue]" in pretty
            general_pretty = run(args.cli, "rows", path, table, "--collections", "--format", "table", "--limit", 1,
                                *(["--stored-calculations"] if cache else []))
            assert general_pretty == pretty
            controls += 1
            controls += 1
            if not cache:
                catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
                start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
                fields = catalog[start].split("\t")
                ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")]) for cells in (line.split("\t") for line in catalog[start + 1:])}
                declarations = run(args.cli, "inspect", path, "--table", table, "--complex").splitlines()[2:]
                first = declarations[0].split("\t")
                flat, foreign = first[3:5]
                row = roots(original, ids[flat])[0][0]
                offset, _, _ = column_layout(original, ids[flat], foreign)
                changed = bytearray(original)
                changed[row + 2 + offset:row + 6 + offset] = (999).to_bytes(4, "little")
                copy = Path(directory) / "orphan.accdb"
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "rows", str(copy), table, "--multivalues", "--offset", "4294967295"], capture_output=True)
                assert result.returncode == 2 and not result.stdout
                general = subprocess.run([str(args.cli), "rows", str(copy), table, "--collections", "--offset", "4294967295"], capture_output=True)
                assert general.returncode == 2 and not general.stdout
                controls += 2
            assert path.read_bytes() == original
            print(path.name, len(rows), "parent rows; size", len(original), "unchanged SHA-256", hashlib.sha256(original).hexdigest())
    print(controls, "mixed-page, null/empty, stale cache, refusal, pretty and corruption controls passed; native Access unrun")


if __name__ == "__main__":
    main()
