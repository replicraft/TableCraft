"""Metadata-only attachment queries over valid files beyond the payload envelope.

Jackcess supplies inert metadata and verifies its own original file encoding.
Only TableCraft executes SQL. No native Access behavior or save is qualified.
"""
import argparse
import csv
import datetime
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256


INPUT_HASHES = {
    "V2016": "70bba6653108d52d3ec931a749a77148eb44c2f9be8d0b87025020c64082e112",
    "V2019": "593904e7db7c48aa94a1f03bfae4b3fd4d4e02665efc3953d65aed21a81f2813",
    "V2007": "d0b61260ff5a6d669141d7f5b04d88ff0cbf313230c6fef1912bd389abd3930a",
}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    profiles = ["V2016", "V2019", "V2007"]
    if args.generate:
        args.directory.mkdir(exist_ok=False)
    controls = 0
    inventory = {}

    def java(*arguments):
        return subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "AttachmentMetadataFixture.java"), *map(str, arguments),
        ], text=True, encoding="utf-8")

    def query(path, sql, options=(), accepted=True, diagnostic=None):
        nonlocal controls
        result = subprocess.run([
            str(args.cli), "query", "run", sql, "--database", str(path),
            "--collections", "--format", "tsv", *options,
        ], capture_output=True, text=True, encoding="utf-8", check=False)
        if accepted:
            assert result.returncode == 0 and not result.stderr, (sql, result)
        else:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            if diagnostic is not None:
                assert diagnostic in result.stderr, (sql, result)
        controls += 1
        return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))

    for profile in profiles:
        path = args.directory / f"metadata-{profile}.accdb"
        if args.generate:
            print(java(path, profile).strip(), flush=True)
        original = digest(path)
        if not args.generate:
            assert original == INPUT_HASHES[profile], (profile, original)
        rows = [line.split("\t") for line in java("--inspect", path).splitlines()]
        assert [row for row in rows if row[0] == "P"] == [["P", "1", "2"], ["P", "2", "0"]]
        attachments = [row for row in rows if row[0] == "A"]
        assert len(attachments) == 2
        assert int(attachments[0][7]) > 2 * 1024 * 1024
        assert int(attachments[0][8]) == 3 * 1024 * 1024
        expected = []
        for row in attachments:
            values = []
            for cell in row[2:7]:
                if cell == "N":
                    values.append(None)
                elif cell.startswith("T"):
                    values.append(bytes.fromhex(cell[1:]).decode("utf-8"))
                elif cell.startswith("D"):
                    values.append(datetime.datetime.fromisoformat(cell[1:]))
                else:
                    assert cell.startswith("I")
                    values.append(int(cell[1:]))
            expected.append([1, *values])
        expected.append([2, None, None, None, None, None])
        sql = "SELECT ID,Files.FileURL,Files.FileName,Files.FileType,Files.FileTimeStamp,Files.FileFlags FROM MetadataFiles ORDER BY ID"
        for options in [[], ["--decoded-attachments"]]:
            actual = query(path, sql, options)
            assert len(actual) == 4
            for row, wanted in zip(actual[1:], expected):
                parsed = [int(row[0])]
                for index, value in enumerate(row[1:], 1):
                    if value == "NULL":
                        parsed.append(None)
                    elif index == 4:
                        parsed.append(datetime.datetime.fromisoformat(value.strip("#")))
                    elif index == 5:
                        parsed.append(int(value))
                    else:
                        parsed.append(json.loads(value))
                assert parsed == wanted, (profile, parsed, wanted)
        counts = query(path, "SELECT ID,Count(Files.FileName) AS N FROM MetadataFiles GROUP BY ID ORDER BY ID")
        assert counts[1:] == [["1", "2"], ["2", "0"]]
        assert len(query(path, "SELECT TOP 1 Files.FileName FROM MetadataFiles")) == 2
        query(
            path, "SELECT Files.FileData FROM MetadataFiles",
            ["--decoded-attachments"], accepted=False,
            diagnostic="query structural limit exceeded",
        )
        query(
            path, "SELECT Files.FileName,Files.FileData FROM MetadataFiles WHERE ID=2",
            ["--decoded-attachments"], accepted=False,
            diagnostic="query structural limit exceeded",
        )
        query(path, "SELECT Files.FileName FROM MetadataFiles", ["--index", "MetadataFiles", "IdAsc"], accepted=False)
        assert digest(path) == original
        inventory[profile] = {"sha256": original, "bytes": path.stat().st_size,
                              "encoded_bytes": int(attachments[0][7]), "file_sha256": attachments[0][9]}

    print(json.dumps(inventory, indent=2))
    print(f"Passed {controls} independent metadata/payload-boundary controls; native Access unrun")


if __name__ == "__main__":
    main()
