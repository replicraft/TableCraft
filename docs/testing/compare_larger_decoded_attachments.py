"""Check explicit native attachment file allowances against original inert data."""

import argparse
import csv
import hashlib
import json
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest

RECIPES = Path(__file__).resolve().parent
PROFILES = {"2007": "V2007", "2016": "V2016", "2019": "V2019"}
INPUT_HASHES = {
    "2007": "4c66295908cef308d703f7a07be42524e40db1ef038ca2270bbd37eb87f3ae4a",
    "2016": "7fbe870ba2eb4216beb63b3a0aff99fd1efe20598a63bf5a4b7a738bdbd77e9c",
    "2019": "e2ef569a4088c656567d68b67a634bded5cef6e7b57790cb7a5c7f8015486302",
}


def changed(options, flag, value):
    result = list(options)
    result[result.index(flag) + 1] = value
    return result


def query(cli, path, sql, options, accepted=True):
    result = subprocess.run(
        [str(cli), "query", "run", sql, "--database", str(path), *options],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    assert result.returncode == (0 if accepted else 2), (sql, result.stderr)
    if not accepted:
        assert not result.stdout, result.stdout[:100]
        assert "limit" in result.stderr.lower(), result.stderr
        return None
    assert not result.stderr, result.stderr
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    csv.field_size_limit(16 * 1024 * 1024)
    if args.generate:
        args.directory.mkdir(parents=True, exist_ok=False)

    files = {
        1: ("large.txt", bytes([42]) * 3_145_728, 1),
        2: ("stored.zip", bytes((index * 17 + index // 256) % 256 for index in range(1_500_000)), 0),
    }
    pools = [
        "--native-bytes", "134217728", "--native-steps", "100000000",
        "--execution-bytes", "134217728", "--execution-steps", "100000000",
    ]
    decoded = ["--collections", "--decoded-attachments"]
    large = [*pools, *decoded, "--attachment-bytes", "3145728", "--output-bytes", "16777216"]
    sql = "SELECT ID,Files.FileName AS Label,Files.FileData AS Payload FROM LargeFiles ORDER BY ID"
    controls = 0

    with tempfile.TemporaryDirectory(prefix="tablecraft-larger-file-getters-") as temporary:
        subprocess.run(
            ["javac", "-cp", str(args.jar), "-d", temporary, str(RECIPES / "LargeDecodedAttachmentFixture.java")],
            check=True,
        )
        java = [
            "java", "-Dstdout.encoding=UTF-8", "-cp",
            os.pathsep.join((str(args.jar), temporary)), "LargeDecodedAttachmentFixture",
        ]
        for suffix, profile in PROFILES.items():
            path = args.directory / ("files-" + suffix + ".accdb")
            if args.generate:
                subprocess.run([*java, str(path), profile], check=True)
            original = digest(path)
            if not args.generate:
                assert original == INPUT_HASHES[suffix], (path, original)
            getter = subprocess.check_output([*java, "--inspect", str(path)], text=True, encoding="utf-8")
            observed = set()
            for line in getter.splitlines():
                identifier, name, length, checksum, encoding, encoded_length = line.split("\t")
                assert int(identifier) not in observed
                observed.add(int(identifier))
                expected_name, data, expected_encoding = files[int(identifier)]
                assert (name, int(length), checksum, int(encoding)) == (
                    expected_name, len(data), hashlib.sha256(data).hexdigest(), expected_encoding,
                )
                assert int(encoded_length) <= 2_097_152
                controls += 1
            assert observed == set(files)

            output = query(args.cli, path, sql, large)
            assert output[0] == ["ID", "Label", "Payload"]
            assert output[-1] == ["3", "NULL", "NULL"]
            assert len(output) == 4
            for row in output[1:-1]:
                name, expected, _ = files[int(row[0])]
                assert json.loads(row[1]) == name
                assert row[2].startswith("0x")
                assert bytes.fromhex(row[2][2:]) == expected
            controls += 1

            larger = [*pools, *decoded, "--attachment-bytes", "16777216"]
            assert query(args.cli, path, "SELECT Count(Files.FileData) AS N FROM LargeFiles", larger) == [["N"], ["2"]]
            metadata = [*pools, *decoded, "--attachment-bytes", "1"]
            assert query(args.cli, path, "SELECT ID,Files.FileName AS Label FROM LargeFiles ORDER BY ID", metadata) == [
                ["ID", "Label"], ["1", '"large.txt"'], ["2", '"stored.zip"'], ["3", "NULL"],
            ]
            assert query(args.cli, path, "SELECT ID FROM LargeFiles ORDER BY ID", metadata) == [["ID"], ["1"], ["2"], ["3"]]
            controls += 3

            for options in [
                [*pools, *decoded],
                [*pools, *decoded, "--attachment-bytes", "3145727"],
                changed(large, "--output-bytes", "1024"),
                changed(large, "--native-bytes", "1000000"),
                changed(large, "--execution-bytes", "1000000"),
            ]:
                query(args.cli, path, sql, options, accepted=False)
                controls += 1
            assert digest(path) == original
            print(path.name, path.stat().st_size, original, "passed", flush=True)

    print(f"Passed {controls} independent larger decoded-file controls; native Access unrun.")


if __name__ == "__main__":
    main()
