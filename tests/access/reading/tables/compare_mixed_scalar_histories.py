"""Compare mixed scalar row histories with Python models and inert getters.

Original recipes and reviewed Jackcess getters establish independent reading,
not Microsoft Access SQL, native save/reopen or coherent live-file behavior.
"""

import argparse
import csv
import datetime
import hashlib
from io import StringIO
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES, display
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2016": "22ae1aa4a504a2540f45946997fdbe9e64703263df3e08068c2855fa7b69d937",
    "V2007": "697796edb15b81784e479cb64d0bb144d5182dc0d49b1d10af0856cdb1db32c8",
    "V2000": "4fe3db9bdc32b8c552b20d72dcb4438372654adca37e9208850f2811677dc25d",
}
PROFILES = [("V2016", "accdb", 5), ("V2007", "accdb", 2), ("V2000", "mdb", 1)]
COLUMNS = ["ID", "Flag", "Octet", "Small", "Money", "Real", "WideReal", "Label",
           "Notes", "Bytes", "Opaque", "G", "Amount", "Stamp", "Later", "LaterText", "LaterMoney"]
NATIVE = ["--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "100000000"]
EXECUTION = ["--execution-bytes", "268435456", "--execution-rows", "1000000", "--execution-steps", "100000000"]


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def binary(identity, length):
    return "X" + bytes((identity + position * 17) % 256 for position in range(length)).hex()


def text(value):
    return "T" + value.encode().hex()


def model():
    result = {}
    positions = {position * 7_919 % 20_000 + 1: position for position in range(20_000)}
    for identity in range(1, 20_101):
        appended = identity > 20_000
        if not appended and identity % 101 == 0:
            continue
        updated = not appended and identity % 97 == 0
        later = appended or updated or positions[identity] >= 10_000
        absent = identity % 13 == 0
        signed = identity if identity % 2 == 0 else -identity
        notes = f"grown:{identity}:" + "é😀" * 256 if updated else f"n:{identity}"
        stamp = 45_292.5 + identity % 365 if identity % 2 == 0 else -1.25
        row = [f"I32:{identity}", "B1" if identity % 3 == 0 else "B0"]
        row.extend([
            f"U{identity % 256}", f"I16:{identity % 65_536 - 32_768}",
            f"C{signed * 10_001}",
            "F" + struct.pack(">f", -0.0 if identity % 17 == 0 else signed / 8).hex(),
            "D" + struct.pack(">d", -0.0 if identity % 19 == 0 else signed / 16).hex(),
            text(f"label:{identity}:é中😀"), text(notes), binary(identity, identity % 17),
            binary(identity, 8_193 if updated else identity % 67),
            f"G{identity:08x}123456789abc{identity * 17:012x}",
            f"Q{900719925474099300000000 + signed}:4",
            "Y64:" + struct.pack(">d", stamp).hex(),
        ])
        if absent:
            row[2:] = ["N"] * 12
        row.extend([
            f"I32:{identity * 2}", text(("updated:" if updated else "later:") + str(identity)),
            f"C{-signed * 3}",
        ] if later and not absent else ["N"] * 3)
        result[identity] = row
    assert len(result) == 19_902
    return result


def rendered(tag):
    if tag.startswith("Y64:"):
        serial = struct.unpack(">d", bytes.fromhex(tag[4:]))[0]
        # Negative OLE DATE fractions describe positive time-of-day on that date.
        days = int(serial)
        fraction = abs(serial - days)
        date = datetime.datetime(1899, 12, 30) + datetime.timedelta(days=days, seconds=fraction * 86_400)
        return "#" + date.strftime("%Y-%m-%d %H:%M:%S") + ".000000000#"
    return display(tag)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "rows", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed Jackcess artifact"
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, extension, _ in PROFILES:
            subprocess.run([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "MixedScalarHistoryFixture.java"),
                str(args.directory / f"mixed-{profile}.{extension}"), profile,
            ], check=True)

    expected = model()
    controls = 0
    for profile, extension, family in PROFILES:
        path = args.directory / f"mixed-{profile}.{extension}"
        before = digest(path)
        if not args.generate:
            assert before == INPUT_HASHES[profile], (profile, "Unregistered fixture")
        with path.open("rb") as source:
            assert source.read(24)[20:24] == bytes([family, 0, 0, 0]), "Wrong header family"
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "--date-bits", "MixedHistory",
        ], text=True, encoding="utf-8")
        records = [line.split("\t")[1:] for line in reference.splitlines()]
        identifiers = [int(row[0][4:]) for row in records]
        assert len(identifiers) == len(set(identifiers)) == len(expected), (profile, "count/identity")
        for row, identity in zip(records, identifiers):
            assert row == expected[identity], (profile, identity, "independent stored value")
        controls += 1

        for batch in [127, 257]:
            actual = subprocess.check_output([
                str(args.rows), str(path), "MixedHistory", "--date-bits", "--batch-size", str(batch),
                "--bytes", "268435456", "--steps", "100000000",
            ], text=True, encoding="utf-8")
            assert actual == reference, (profile, batch, "complete tagged cursor")
            controls += 1
        selected = [0, 5, 9, 11, 12, 13, 16]
        projected = subprocess.check_output([
            str(args.rows), str(path), "MixedHistory", "--date-bits", "--batch-size", "131",
            "--bytes", "268435456", "--steps", "100000000",
            *[argument for position in reversed(selected) for argument in ["--column", COLUMNS[position]]],
        ], text=True, encoding="utf-8")
        wanted = "".join("MixedHistory\t" + "\t".join(row[position] for position in selected) + "\n" for row in records)
        assert projected == wanted, (profile, "projected tagged cursor")
        controls += 1

        def command(arguments, expected_rows=None, diagnostic=None):
            nonlocal controls
            result = subprocess.run([str(args.cli), *arguments], capture_output=True, text=True, encoding="utf-8")
            if diagnostic:
                assert result.returncode == 2 and not result.stdout, (profile, "non-atomic refusal")
                assert diagnostic in result.stderr, (profile, result.stderr)
            else:
                assert result.returncode == 0 and not result.stderr, (profile, result.stderr)
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual == expected_rows, (profile, arguments[:3], "displayed values differ")
            controls += 1

        displayed = [[rendered(tag) for tag in row] for row in records]
        command(["rows", str(path), "MixedHistory", "--stream", "--limit", "257", *NATIVE],
                [COLUMNS, *displayed])
        command(["rows", str(path), "MixedHistory", "--offset", "1", "--limit", "17"],
                [COLUMNS, *displayed[1:18]])

        def query(sql, expected_rows=None, options=(), diagnostic=None):
            command(["query", "run", sql, "--database", str(path), *options], expected_rows, diagnostic)

        query("SELECT TOP 31 * FROM MixedHistory ORDER BY ID DESC",
              [COLUMNS, *[[rendered(tag) for tag in expected[identity]] for identity in sorted(expected, reverse=True)[:31]]],
              NATIVE + EXECUTION)
        grown = [identity for identity in sorted(expected, reverse=True) if identity <= 20_000 and identity % 97 == 0][:17]
        query("SELECT TOP 17 * FROM MixedHistory WHERE ID<=20000 AND ID Mod 97=0 ORDER BY ID DESC",
              [COLUMNS, *[[rendered(tag) for tag in expected[identity]] for identity in grown]], NATIVE + EXECUTION)
        query("SELECT Count(*) AS N,Count(LaterMoney) AS L,Sum(ID) AS S FROM MixedHistory",
              [["N", "L", "S"], [str(len(expected)), str(sum(row[-1] != "N" for row in records)), str(sum(expected))]],
              NATIVE + EXECUTION)
        money = sum(int(row[4][1:]) for row in records if row[4] != "N")
        later_money = sum(int(row[-1][1:]) for row in records if row[-1] != "N")
        query("SELECT Sum([Money]) AS M,Sum(LaterMoney) AS L FROM MixedHistory",
              [["M", "L"], [display(f"C{money}"), display(f"C{later_money}")]], NATIVE + EXECUTION)
        years = {"NULL": 0, "1899": 0, "2024": 0}
        for identity in expected:
            year = "NULL" if identity % 13 == 0 else "2024" if identity % 2 == 0 else "1899"
            years[year] += 1
        query("SELECT Year(Stamp) AS Y,Count(*) AS N FROM MixedHistory GROUP BY Year(Stamp) ORDER BY Year(Stamp)",
              [["Y", "N"], *[[year, str(count)] for year, count in years.items()]], NATIVE + EXECUTION)
        query("SELECT Count(*) AS N FROM MixedHistory WHERE [Money] IS NULL AND G IS NULL AND Stamp IS NULL",
              [["N"], [str(sum(identity % 13 == 0 for identity in expected))]], NATIVE + EXECUTION)
        query("SELECT Count(*) AS N FROM MixedHistory WHERE ID<=20000 AND ID Mod 101=0",
              [["N"], ["0"]], NATIVE + EXECUTION)
        query("SELECT Count(*) FROM MixedHistory", options=EXECUTION, diagnostic="limit")
        query("SELECT TOP 1 * FROM MixedHistory", options=NATIVE[2:] + EXECUTION + ["--native-bytes", "1"], diagnostic="limit")
        assert digest(path) == before, (profile, "Input changed")
        print(f"{profile}: {len(expected)} retained mixed rows; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} mixed scalar history controls; native Access unrun.")


if __name__ == "__main__":
    main()
