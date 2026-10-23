"""Independent larger attachment/history models and exact bounded reader checks."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2016": "49877cf2cd236cb602dc1eaf2598d0fac81f0caeced02a137f3f48bb3db8d60c",
    "V2007": "f620120c1b030698005b93eee5a27500ad7bc924b4f16f4be19d64615f5785a7",
}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def date_bits(child):
    return "Y64:" + struct.pack(">d", 45292.5 + child % 7).hex()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "rows", "attachments", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    profiles = ["V2016", "V2007"]
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile in profiles:
            subprocess.run([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "LargeStoredObjectsFixture.java"),
                str(args.directory / f"stored-{profile}.accdb"), profile,
            ], check=True)

    native = ["--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "100000000"]
    execution = ["--execution-bytes", "268435456", "--execution-rows", "1000000", "--execution-steps", "100000000"]
    engineering = ["--bytes", "268435456", "--steps", "100000000"]
    controls = 0
    for binary, flag, mode in [
        (args.rows, "--collection-rows", ["Unused", "--version-history", "Notes"]),
        (args.attachments, "--rows", []),
    ]:
        for value in ["0", "1000001", "-1"]:
            result = subprocess.run([str(binary), "absent.accdb", *mode, flag, value], text=True, capture_output=True)
            assert result.returncode != 0 and not result.stdout and "error:" in result.stderr
            controls += 1
    result = subprocess.run([str(args.rows), "absent.accdb", "Unused", "--collection-rows", "1000000"], text=True, capture_output=True)
    assert result.returncode != 0 and not result.stdout and "require" in result.stderr
    controls += 1
    for profile in profiles:
        path = args.directory / f"stored-{profile}.accdb"
        before = digest(path)
        if not args.generate:
            assert before == INPUT_HASHES[profile], (profile, "unregistered fixture")
        with path.open("rb") as source:
            prefix = source.read(24)
        assert prefix[20:24] == bytes([5 if profile == "V2016" else 2, 0, 0, 0])

        def java(recipe, *arguments):
            return subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / recipe), *map(str, arguments),
            ], text=True, encoding="utf-8")

        metadata = java("LargeAttachmentOracle.java", "--metadata", path)
        decoded = java("LargeAttachmentOracle.java", "--decoded", path)
        meta_rows = [line.split("\t") for line in metadata.splitlines()]
        data_rows = [line.split("\t") for line in decoded.splitlines()]
        assert len(meta_rows) == len(data_rows) == 20_000
        assert len({row[1] for row in meta_rows}) == 20_000
        for position, (meta, data) in enumerate(zip(meta_rows, data_rows)):
            parent, child = divmod(position, 100)
            assert meta[:2] == data[:2] and meta[0] == str(parent)
            assert meta[2:5] == ["T", "T" + f"f{parent + 1}-{child}.bin".encode().hex(), "T62696e"]
            assert meta[5].startswith("X") and len(meta[5]) > 7
            assert meta[6:] == [date_bits(child), f"I32:{child % 3}"]
            assert data[2:] == ["62696e", bytes([parent + 1, child, 255]).hex()]

        history = java("VersionHistoryOracle.java", path, "Histories", "Notes")
        histories = [line.split("\t") for line in history.splitlines()]
        assert len(histories) == 201
        history_ids = set()
        for parent, row in enumerate(histories):
            assert row[0:2] == ["Histories", str(parent)]
            assert int(row[2]) > 0
            assert len(row) == (303 if parent < 200 else 3)
            for child in range(100 if parent < 200 else 0):
                identity, text, timestamp = row[3 + child * 3:6 + child * 3]
                assert identity.startswith("I32:") and identity not in history_ids
                history_ids.add(identity)
                expected = "N" if child % 13 == 0 else "T" + f"v:{parent + 1}:{child % 7}:é😀".encode().hex()
                assert text == expected
                assert timestamp == ("N" if child % 11 == 0 else date_bits(child))
        assert len(history_ids) == 20_000

        def exact(command, expected):
            nonlocal controls
            result = subprocess.run(list(map(str, command)), text=True, encoding="utf-8", capture_output=True)
            assert result.returncode == 0 and not result.stderr, (profile, result.stderr)
            assert result.stdout == expected, (profile, "tagged values differ")
            controls += 1

        attachment_command = [args.attachments, path, "--table", "ManyFiles", *engineering]
        exact([*attachment_command, "--rows", "1000000", "--metadata"], metadata)
        exact([*attachment_command, "--rows", "1000000"], decoded)
        history_command = [args.rows, path, "Histories", "--version-history", "Notes", *engineering]
        exact([*history_command, "--collection-rows", "1000000"], history)
        for command in [attachment_command, history_command]:
            result = subprocess.run(list(map(str, command)), text=True, capture_output=True)
            assert result.returncode != 0 and not result.stdout and "limit" in result.stderr.lower()
            controls += 1

        def query(sql, expected=None, options=(), accepted=True):
            nonlocal controls
            result = subprocess.run([
                str(args.cli), "query", "run", sql, "--database", str(path), *options,
            ], text=True, encoding="utf-8", capture_output=True)
            if accepted:
                assert result.returncode == 0 and not result.stderr, (profile, sql, result.stderr)
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual[1:] == expected, (profile, sql, actual, expected)
            else:
                assert result.returncode == 2 and not result.stdout, (profile, sql, result)
            controls += 1

        collection = ["--collections", *native, *execution]
        query("SELECT Count(*),Count(Files.FileName),Sum(Files.FileFlags) FROM ManyFiles",
              [["20001", "20000", "19800"]], collection)
        query("SELECT Count(Files.FileName) FROM ManyFiles", options=["--collections", *execution], accepted=False)
        query("SELECT Count(Files.FileData) FROM ManyFiles", [["20000"]], [*collection, "--decoded-attachments"])
        query("SELECT TOP 3 ID,Files.FileData AS D,Files.FileFlags AS F FROM ManyFiles",
              [["1", "0x0100ff", "0"], ["1", "0x0101ff", "1"], ["1", "0x0102ff", "2"]],
              [*collection, "--decoded-attachments"])
        query("SELECT ID FROM ManyFiles WHERE Files.FileData IS NULL", [],
              [*collection, "--decoded-attachments"])
        query("SELECT ID,Files.FileData AS D FROM ManyFiles WHERE Files.FileData IS NULL", [["201", "NULL"]],
              [*collection, "--decoded-attachments"])
        query("SELECT Count(Files.FileData) FROM ManyFiles", options=collection, accepted=False)
        query("SELECT Count(Notes) FROM Histories", [["0"]], [*native, *execution])
        assert digest(path) == before
        print(f"{profile}: exact 20,000 attachments/versions and 201 parents per table; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} larger stored-object controls; native Access unrun.")


if __name__ == "__main__":
    main()
