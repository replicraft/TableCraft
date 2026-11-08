"""Check one registered genuine V2000 source through normal journal UPDATE.

Uses original repository ScalarFixture data generated with reviewed Jackcess.
Only fresh temporary copies are edited. This checks existing CLI software and
complete bytes, not Microsoft Access execution or physical durability.
"""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tempfile

from cli_tsv import display_tsv

SOURCE_BYTES = 135168
SOURCE_SHA256 = "de2e67a9f0194988d031cc9c981176ae9a76d3bf80599040729b408fc2b7be82"
EXPECTED_SHA256 = "abf76b9a2eec8ffd8074a2eb131cebe48d17b75691217b6e475292074108e54f"
ID_OFFSET = 130954
COLUMNS = [
    "ID", "Flag", "Octet", "Small", "Money", "Real", "WideReal",
    "Label", "Notes", "Bytes", "Opaque", "G", "Amount", "Stamp",
]
REPORT = (
    "Transaction\tCommitted\n"
    "Matched rows\t1\n"
    "Changed rows\t1\n"
    "Journal namespace\tRetired\n"
    "Retirement guarantee\tCaptured-parent barrier\n"
    "Journal cleanup\tRemoved\n"
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(cli, *arguments):
    result = subprocess.run(
        [str(cli), *map(str, arguments)], capture_output=True, text=True,
        encoding="utf-8", check=True,
    )
    if result.stderr:
        raise RuntimeError("Unexpected diagnostic from successful journal comparison")
    return result.stdout


def expected_image(original):
    if len(original) != SOURCE_BYTES or digest(original) != SOURCE_SHA256:
        raise ValueError("Requires the registered 23 October V2000 ScalarFixture source")
    # Literal DATA page/slot and fourteen-column first-row ID. These values are
    # independent of the production reader, record layout and mutation encoder.
    page = original[31 * 4096:32 * 4096]
    if page[:2] != b"\x01\x01" or page[14:16] != b"\x88\x0f":
        raise ValueError("Independent first-row physical locator differs")
    if original[ID_OFFSET - 2:ID_OFFSET + 4] != b"\x0e\x00\x01\x00\x00\x00":
        raise ValueError("Independent original first-row ID differs")
    expected = bytearray(original)
    expected[ID_OFFSET:ID_OFFSET + 4] = b"\x29\x00\x00\x00"
    if digest(expected) != EXPECTED_SHA256:
        raise ValueError("Independent complete expected image differs")
    return bytes(expected)


def checked_rows(cli, path):
    schema = display_tsv(run(cli, "inspect", path, "--table", "ScalarSamples"))
    if "ID\t4\t4\t0x03\t0x00\tInt32\n" not in schema:
        raise RuntimeError("Source does not expose the expected ordinary Int32 ID")
    indexes = display_tsv(run(cli, "inspect", path, "--table", "ScalarSamples", "--indexes"))
    if len(indexes.splitlines()) != 2:
        raise RuntimeError("Source has indexes outside this unindexed comparison")
    checked = display_tsv(run(cli, "inspect", path, "--table", "ScalarSamples", "--check"))
    if "Rows checked\t3\n" not in checked or "Indexes not checked\t0\n" not in checked:
        raise RuntimeError("Source did not complete its three-row table check")
    text = run(cli, "rows", path, "ScalarSamples")
    rows = list(csv.reader(io.StringIO(text), delimiter="\t"))
    if len(rows) != 4 or rows[0] != COLUMNS or any(len(row) != len(COLUMNS) for row in rows):
        raise RuntimeError("Unexpected complete source row/schema shape")
    return rows


def compare(cli, database):
    original = database.read_bytes()
    expected = expected_image(original)
    with tempfile.TemporaryDirectory(prefix="tablecraft-jet4-journal-") as directory:
        root = Path(directory)
        candidate = root / "independent-expected.mdb"
        candidate.write_bytes(expected)
        # Public-reader SOURCE gates precede the first command that can write.
        before_rows = checked_rows(cli, database)
        if [row[0] for row in before_rows[1:]] != ["1", "2", "3"]:
            raise RuntimeError("Unexpected original sequential ID values")
        expected_rows = [list(row) for row in before_rows]
        expected_rows[1][0] = "41"
        if checked_rows(cli, candidate) != expected_rows:
            raise RuntimeError("Public-reader expected rows differ from independent ID change")
        copy = root / "journal-copy.mdb"
        copy.write_bytes(original)
        journal = root / "fresh-journal"
        report = run(
            cli, "query", "journal-update",
            "UPDATE ScalarSamples SET ID=41 WHERE ID=1",
            "--database", copy, "--journal", journal,
        )
        if display_tsv(report) != REPORT:
            raise RuntimeError("Unexpected committed journal lifecycle report")
        if copy.read_bytes() != expected:
            raise RuntimeError("Journal output differs from independent complete expected bytes")
        if journal.exists():
            raise RuntimeError("Successful local journal lifecycle left an active bundle")
        if checked_rows(cli, copy) != expected_rows:
            raise RuntimeError("Committed complete rows failed reopen comparison")
    if database.read_bytes() != original:
        raise RuntimeError("Registered original source changed")
    return {
        "source_sha256": SOURCE_SHA256,
        "expected_sha256": EXPECTED_SHA256,
        "bytes": SOURCE_BYTES,
        "changed_offsets": [ID_OFFSET],
        "matched_rows": 1,
        "changed_rows": 1,
        "checked_rows": 3,
        "indexes": 0,
        "original_unchanged": True,
        "native_access_comparison": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--database", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(compare(args.cli.resolve(), args.database), indent=2))


if __name__ == "__main__":
    main()
