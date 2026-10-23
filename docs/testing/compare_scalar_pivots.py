"""Check typed scalar pivot keys and explicit heading names against Python models."""
import argparse
from collections import Counter
import csv
import datetime as dt
from decimal import Decimal
import hashlib
from io import StringIO
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_cstr import date_text
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_numeric_text import floating
from compare_union_types import JAR_SHA256


def fixed_text(value):
    text = format(value, "f")
    return text.rstrip("0").rstrip(".") if "." in text else text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    controls = 0

    def check(sql, names, counts, total, options=(), accepted=True):
        nonlocal controls
        result = subprocess.run([str(args.cli), "query", "run", sql, *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        assert result.returncode == (0 if accepted else 2), (sql, result.stderr)
        if accepted:
            assert not result.stderr
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            cells = [str(count) if count is not None else "NULL" for count in counts]
            expected = [["G", "Total", *names], ["TRUE", str(total), *cells]]
            assert actual == expected, (sql, actual, expected)
        else:
            assert not result.stdout, (sql, result.stdout[:100])
        controls += 1

    rows = "(SELECT 1 AS ID UNION ALL SELECT 2 UNION ALL SELECT 3) AS Q"
    base = f"TRANSFORM Count(*) SELECT True AS G,Count(*) AS Total FROM {rows} GROUP BY True PIVOT "
    for function in ["CSng", "CDbl", "CCur", "CDec"]:
        values = [Decimal(value) / 4 for value in [1, 2, 3]]
        for profile in ["invariant", "de-de"]:
            names = [fixed_text(value) for value in values]
            if profile == "de-de":
                names = [name.replace(".", ",") for name in names]
            check(base + f"{function}(ID/4)", names, [1, 1, 1], 3, ["--number-format", profile])
        check(base + f"{function}(ID/4)", [], [], 0, accepted=False)
    for profile in ["iso", "en-us", "de-de"]:
        epoch = dt.datetime(1899, 12, 30)
        names = [date_text(epoch + dt.timedelta(days=value), profile) for value in [1, 2, 3]]
        check(base + "CDate(ID)", names, [1, 1, 1], 3, ["--date-format", profile])
    check(base + "CDate(ID)", [], [], 0, accepted=False)
    check(base + "(ID Mod 2=0)", ["True", "False"], [1, 2], 3)
    check(
        base + "CCur(ID/4) IN (CCur(0.75),CCur(0.25),CCur(2))",
        ["0.75", "0.25", "2"], [1, 1, None], 3,
        ["--number-format", "invariant"],
    )
    check(
        base + "CDbl(ID) IN (CDbl(1),CDbl(1.0000000000000002))",
        [], [], 0, ["--number-format", "invariant"], accepted=False,
    )
    check(base + "CDbl(1e-300)", [], [], 0, ["--number-format", "invariant"], accepted=False)

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INPUT_HASHES.items():
            path = args.directory / ("index-" + suffix)
            digest = registered_input_hash(path, digest)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"], text=True, encoding="utf-8")
            stored = [line.split("\t")[1:] for line in output.splitlines()]
            assert len(stored) == 20000
            budgets = [
                "--database", path,
                "--native-bytes", "67108864",
                "--native-rows", "100000",
                "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ]
            native = "TRANSFORM Count(*) SELECT True AS G,Count(*) AS Total FROM IndexedRows GROUP BY True PIVOT "
            for column, position, kind in [
                ("Flag", 4, "boolean"),
                ("Money", 5, "currency"),
                ("SingleValue", 6, "single"),
                ("Stamp", 8, "date"),
                ("Amount", 12, "decimal"),
            ]:
                values = []
                for row in stored:
                    tag = row[position]
                    if tag == "N":
                        continue
                    if kind == "boolean":
                        values.append(-1 if tag == "B1" else 0)
                    elif kind == "currency":
                        values.append(Decimal(int(tag[1:])).scaleb(-4))
                    elif kind == "single":
                        values.append(struct.unpack(">f", bytes.fromhex(tag[1:]))[0])
                    elif kind == "date":
                        values.append(dt.datetime.fromisoformat(tag[1:]))
                    else:
                        coefficient, scale = tag[1:].split(":")
                        values.append(Decimal(int(coefficient)).scaleb(-int(scale)))
                counts = Counter(values)
                keys = sorted(counts)
                for profile in (["iso", "de-de"] if kind == "date" else ["invariant", "de-de"]):
                    if kind == "date":
                        names = [date_text(value, profile) for value in keys]
                        formats = ["--date-format", profile]
                    else:
                        if kind == "boolean":
                            names = ["True" if value else "False" for value in keys]
                        elif kind == "single":
                            names = [floating(value, 7).lstrip(" ") for value in keys]
                        else:
                            names = [fixed_text(value) for value in keys]
                        if profile == "de-de":
                            names = [name.replace(".", ",") for name in names]
                        formats = ["--number-format", profile]
                    check(
                        native + f"[{column}]", names,
                        [counts[value] for value in keys], 20000,
                        [*budgets, *formats],
                    )
            # A subnormal Double's normal-format heading exceeds the existing
            # 255-byte name ceiling. A fixed list can omit that key explicitly.
            check(native + "DoubleValue", [], [], 0, [*budgets, "--number-format", "invariant"], accepted=False)
            values = [
                struct.unpack(">d", bytes.fromhex(row[7][1:]))[0]
                for row in stored if row[7] != "N"
            ]
            counts = Counter(values)
            check(
                native + "DoubleValue IN (CDbl(0),CDbl(1e200))",
                ["0", "1E+200"], [counts[0.0], counts[1e200]], 20000,
                [*budgets, "--number-format", "invariant"],
            )
            assert path.read_bytes() == original
    print(f"Passed {controls} independent scalar-heading/crosstab/format/large-native controls; native Access unrun.")


if __name__ == "__main__":
    main()
