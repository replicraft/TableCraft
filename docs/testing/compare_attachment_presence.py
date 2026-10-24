"""Attachment container NULL criteria checked against inert Jackcess membership counts.

Uses registered originals with valid 3 MiB attachments. Only TableCraft evaluates
SQL; this does not qualify native Access SQL or coherent shared-file reads.
"""
import argparse
import csv
from io import StringIO
from pathlib import Path
import subprocess

from compare_attachment_metadata import INPUT_HASHES, digest
from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    controls = 0

    for profile, expected_hash in INPUT_HASHES.items():
        path = args.directory / f"metadata-{profile}.accdb"
        assert digest(path) == expected_hash
        report = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "AttachmentMetadataFixture.java"), "--inspect", str(path),
        ], text=True, encoding="utf-8")
        parents = [line.split("\t") for line in report.splitlines() if line.startswith("P\t")]
        empty = [[identity] for _, identity, count in parents if int(count) == 0]
        present = [[identity] for _, identity, count in parents if int(count) != 0]

        for decoded in [False, True]:
            options = ["--decoded-attachments"] if decoded else []
            for condition, expected in [
                ("Files IS NULL", empty),
                ("Files IS NOT NULL", present),
                ("NOT (Files IS NULL)", present),
                ("Files IS NULL OR Files IS NOT NULL", [[identity] for _, identity, _ in parents]),
                ("Files IS NULL AND Files IS NOT NULL", []),
            ]:
                sql = f"SELECT ID FROM MetadataFiles WHERE {condition} ORDER BY ID"
                result = subprocess.run([
                    str(args.cli), "query", "run", sql, "--database", str(path),
                    "--collections", "--format", "tsv", *options,
                ], capture_output=True, text=True, encoding="utf-8", check=False)
                assert result.returncode == 0 and not result.stderr, (profile, sql, result)
                rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert rows[1:] == expected, (profile, sql, rows, expected)
                controls += 1

        assert digest(path) == expected_hash

    print(f"Passed {controls} independent attachment-presence controls; native Access unrun")


if __name__ == "__main__":
    main()
