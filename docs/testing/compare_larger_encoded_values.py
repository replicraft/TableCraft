"""Compare larger native OLE/FileData reads with inert getters and original bytes."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_attachment_metadata import INPUT_HASHES as ATTACHMENT_HASHES
from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest

RECIPES = Path(__file__).resolve().parent
PROFILES = {"jet4": "V2000", "2007": "V2007", "2016": "V2016", "2019": "V2019"}
INPUT_HASHES = {
    "jet4": "96ea363f3a59715b8ca025bab2a4ecd0a6c810859f95f2e4c17880b9882e09bc",
    "2007": "170c3bebfbab11956e09367af5f51284a35306341a3226b60dded42634a8b74f",
    "2016": "08945ddbccd667ef7ec84b01c050423d63ea34de482edaba5adfe0cf9bcb9257",
    "2019": "957ddc86bf7f175584bde53173dac48ea5514a773193113ac356e46d175e9ff0",
}
POOLS = [
    "--native-bytes", "268435456", "--native-steps", "100000000",
    "--execution-bytes", "268435456", "--execution-steps", "100000000",
    "--output-bytes", "16777216", "--format", "tsv",
]


def changed(options, flag, value):
    result = list(options)
    result[result.index(flag) + 1] = value
    return result


def query(cli, path, sql, options, accepted=True):
    result = subprocess.run(
        [str(cli), "query", "run", sql, "--database", str(path), *options],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if not accepted:
        assert result.returncode == 2 and not result.stdout, (sql, result.stderr)
        assert "limit" in result.stderr.lower(), result.stderr
        return None
    assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))


def java(jar, name, *arguments):
    return subprocess.check_output(
        ["java", "-Dstdout.encoding=UTF-8", "-cp", str(jar), str(RECIPES / name), *map(str, arguments)],
        text=True, encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory", "attachments"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    if args.generate:
        args.directory.mkdir(parents=True, exist_ok=False)
    csv.field_size_limit(16 * 1024 * 1024)
    controls = 0
    inventory = {}
    expected = {size: bytes((index * 17 + index // 256) % 256 for index in range(size))
                for size in [4_194_304, 20_971_520]}
    large = [*POOLS, "--native-value-bytes", "33554432"]
    for suffix, profile in PROFILES.items():
        extension = ".mdb" if suffix == "jet4" else ".accdb"
        path = args.directory / ("encoded-" + suffix + extension)
        if args.generate:
            java(args.jar, "LargerEncodedValueFixture.java", path, profile)
        original = digest(path)
        if not args.generate:
            assert original == INPUT_HASHES[suffix], (suffix, original)
        oracle = java(args.jar, "LargerEncodedValueFixture.java", "--inspect", path)
        wanted = [f"{id}\t{size}\t{hashlib.sha256(expected[size]).hexdigest()}"
                  for id, size in [(1, 4_194_304), (2, 20_971_520)]]
        assert oracle.splitlines() == [*wanted, "3\t0\t" + hashlib.sha256(b"").hexdigest(), "4\t-1\tNULL"]
        count = "SELECT Count(Payload) AS N FROM EncodedValues"
        assert query(args.cli, path, count, large) == [["N"], ["3"]]
        query(args.cli, path, count, POOLS, False)
        query(args.cli, path, count, changed(large, "--native-value-bytes", "4194304"), False)
        query(args.cli, path, count, changed(large, "--native-bytes", "1048576"), False)
        query(args.cli, path, count, changed(large, "--native-steps", "1000"), False)
        result = query(args.cli, path, "SELECT Payload FROM EncodedValues WHERE ID=1", large)
        actual = bytes.fromhex(result[1][0].removeprefix("0x"))
        assert actual == expected[4_194_304]
        query(args.cli, path, "SELECT Payload FROM EncodedValues WHERE ID=2", large, False)
        assert query(args.cli, path, "SELECT Count(*) AS N FROM EncodedValues", POOLS) == [["N"], ["4"]]
        assert digest(path) == original
        inventory[suffix] = {"bytes": path.stat().st_size, "sha256": original}
        controls += 10

    for profile, original in ATTACHMENT_HASHES.items():
        path = args.attachments / f"metadata-{profile}.accdb"
        assert digest(path) == original
        oracle = [line.split("\t") for line in java(
            args.jar, "AttachmentMetadataFixture.java", "--inspect", path).splitlines() if line.startswith("A\t")]
        options = [*POOLS, "--native-value-bytes", "4194304", "--collections", "--decoded-attachments",
                   "--attachment-bytes", "3145728"]
        sql = "SELECT Files.FileData AS Payload FROM MetadataFiles WHERE ID=1"
        result = query(args.cli, path, sql, options)
        assert len(result) == 3
        for row, wanted in zip(result[1:], oracle):
            actual = bytes.fromhex(row[0].removeprefix("0x"))
            assert len(actual) == int(wanted[8])
            assert hashlib.sha256(actual).hexdigest() == wanted[9]
        query(args.cli, path, sql, changed(options, "--native-value-bytes", "2097152"), False)
        query(args.cli, path, sql, changed(options, "--attachment-bytes", "1048576"), False)
        assert query(args.cli, path, "SELECT Count(*) AS N FROM MetadataFiles", POOLS) == [["N"], ["2"]]
        assert digest(path) == original
        controls += 5
    print({"controls": controls, "inputs": inventory})


if __name__ == "__main__":
    main()
