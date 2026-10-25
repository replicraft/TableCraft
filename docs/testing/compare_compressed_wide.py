"""Check original compressed-text grammar streams and explicit derived copies.

Wide high-zero units are decoded through independent pinned Column.read calls.
Same-width stored-payload substitutions are grammar controls, not native Access
or ordinary public-encoder validity evidence. Seed databases remain unchanged.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RECIPE = ROOT / "docs/testing/CompressedWideFixture.java"
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
PROFILES = ("V2016", "V2019")
MDBTOOLS_SOURCE = ROOT / ".cache/tablecraft-fixtures/mdbtools-iconv-6a8a4b0.c"
MDBTOOLS_SHA256 = "05f7d775b5860ca2e0948fbf32a93f7a831c4b2dc8012269010cb0e176f14045"
PAGE_BYTES = 4096
SLOT_DIRECTORY = 14


def cases():
    rows = [(bytes([255, 254, 0, unit, 0]), chr(unit)) for unit in range(1, 256)]
    rows.extend([
        (bytes([255, 254, 65, 0, 66, 0, 67, 0, 0, 68]), "ABCD"),
        (bytes([255, 254, 0, 0, 65]), "A"),
        (bytes([255, 254, 0, 62, 216, 128, 221, 65, 0]), "🦀A"),
        (bytes([255, 254, 0]) + bytes([65, 0]) * 127, "A" * 127),
    ])
    return rows


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(*arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True,
                            text=True, timeout=240)
    assert result.returncode == 0, (arguments, result.stderr)
    return result.stdout


def tsv(output):
    return list(csv.reader(io.StringIO(output), delimiter="\t", strict=True))


def display(value):
    escaped = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
    body = "".join(escaped.get(character,
                   f"\\u{{{ord(character):04x}}}" if ord(character) < 32
                   or 127 <= ord(character) <= 159 else character) for character in value)
    return '"' + body + '"'


def row_range(data, page, slot):
    base = page * PAGE_BYTES
    directory = base + SLOT_DIRECTORY + slot * 2
    start = base + int.from_bytes(data[directory:directory + 2], "little")
    end = base + (PAGE_BYTES if slot == 0 else
                  int.from_bytes(data[directory - 2:directory], "little"))
    return start, end


def field_ranges(data, page, slot):
    start, end = row_range(data, page, slot)
    assert int.from_bytes(data[start:start + 2], "little") == 3
    count_at = end - 3  # Independent three-column NULL mask plus variable count.
    assert data[end - 1] == 7
    assert int.from_bytes(data[count_at:count_at + 2], "little") == 2
    offsets = [int.from_bytes(data[count_at - 2 * (index + 1):count_at - 2 * index], "little")
               for index in range(3)]
    assert offsets[0] == 6 and offsets[0] <= offsets[1] <= offsets[2]
    return [(start + offsets[index], start + offsets[index + 1]) for index in range(2)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    generation = parser.add_mutually_exclusive_group()
    generation.add_argument("--generate", action="store_true")
    generation.add_argument("--derive", action="store_true",
                            help="Create fresh grammar copies from existing unchanged seeds")
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    assert digest(MDBTOOLS_SOURCE) == MDBTOOLS_SHA256
    streams = cases()
    corpus = args.directory / "streams.tsv"
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        corpus.write_text("".join(f"{index}\t{encoded.hex()}\n"
                                  for index, (encoded, _) in enumerate(streams, 1)))
    else:
        assert corpus.read_text() == "".join(f"{index}\t{encoded.hex()}\n"
                                            for index, (encoded, _) in enumerate(streams, 1))
    paths = {
        args.cli, args.jar, RECIPE, Path(__file__), corpus, MDBTOOLS_SOURCE,
        ROOT / "crates/engines/tablecraft-ace/src/format/text.rs",
        ROOT / "crates/engines/tablecraft-ace/src/table/record.rs",
        ROOT / "crates/engines/tablecraft-ace/src/table/long_text.rs",
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    cells = 0
    copies = []
    for profile in PROFILES:
        original = args.directory / f"seed-{profile}.accdb"
        copy = args.directory / f"grammar-{profile}.accdb"
        if args.generate:
            execute("java", "-cp", args.jar, RECIPE, "--create", original, profile, corpus)
        seed_hash = digest(original)
        locations = [line.split("\t") for line in execute(
            "java", "-cp", args.jar, RECIPE, "--rows", original,
        ).splitlines()]
        assert len(locations) == len(streams)
        decoded = [line.split("\t") for line in execute(
            "java", "-cp", args.jar, RECIPE, "--decode", original, corpus,
        ).splitlines()]
        assert decoded == [["D", str(index), wanted.encode().hex(), wanted.encode().hex()]
                           for index, (_, wanted) in enumerate(streams, 1)]
        controls += len(streams)
        data = bytearray(original.read_bytes())
        ranges = []
        for index, (location, (encoded, _)) in enumerate(zip(locations, streams), 1):
            tag, row_id, page, slot, short, memo = location
            assert tag == "R" and int(row_id) == index
            seed = "Q" * (len(encoded) - 2)
            assert short == memo == seed.encode().hex()
            selected = field_ranges(data, int(page), int(slot))
            (short_start, short_end), (memo_start, memo_end) = selected
            old = bytes([255, 254]) + seed.encode()
            assert data[short_start:short_end] == old
            tagged = int.from_bytes(data[memo_start:memo_start + 4], "little")
            assert tagged & 0x3fffffff == len(old)
            if tagged >> 30 == 2:
                payload_start, payload_end = memo_start + 12, memo_end
            else:
                assert tagged >> 30 == 1 and memo_end - memo_start == 12
                reference = int.from_bytes(data[memo_start + 4:memo_start + 8], "little")
                value_page, value_slot = reference >> 8, reference & 255
                assert data[value_page * PAGE_BYTES + 4:value_page * PAGE_BYTES + 8] == b"LVAL"
                payload_start, payload_end = row_range(data, value_page, value_slot)
            assert data[payload_start:payload_end] == old
            data[short_start:short_end] = encoded
            data[payload_start:payload_end] = encoded
            ranges.append((short_start, short_end, payload_start, payload_end))
        if args.generate or args.derive:
            assert not copy.exists(), "Fresh derived-copy path required"
            copy.write_bytes(data)
        else:
            assert copy.read_bytes() == data
        getter_rows = [line.split("\t") for line in execute(
            "java", "-cp", args.jar, RECIPE, "--rows", copy,
        ).splitlines()]
        assert [row[4:] for row in getter_rows] == [
            [wanted.encode().hex()] * 2 for _, wanted in streams
        ]
        controls += 1
        expected = [[str(index), display(wanted), display(wanted)]
                    for index, (_, wanted) in enumerate(streams, 1)]
        output = tsv(execute(args.cli, "rows", copy, "Streams", "--limit", "1000", "--format", "tsv"))
        assert output == [["ID", "Short", "Memo"], *expected]
        controls += 1
        cells += len(expected) * 3
        for collation in ("ordinal", "database"):
            output = tsv(execute(
                args.cli, "query", "run", "SELECT ID,[Short],[Memo] FROM Streams",
                "--database", copy, "--collation", collation, "--format", "tsv",
            ))
            assert output == [["ID", "Short", "Memo"], *expected]
            controls += 1
            cells += len(expected) * 3
        for extra in (["--native-text-bytes", "0"], ["--output-bytes", "0"]):
            refused = subprocess.run(list(map(str, [args.cli, "query", "run",
                "SELECT [Memo] FROM Streams", "--database", copy, *extra,
                "--format", "tsv"])), capture_output=True, timeout=240)
            assert refused.returncode != 0 and not refused.stdout
            controls += 1
        for column, start, end, other in (
                ("Short", *ranges[-1][:2], "Memo"),
                ("Memo", *ranges[-1][2:], "Short")):
            malformed = bytearray(data)
            malformed[end - 1] = 0xd8  # Last wide unit becomes an unpaired surrogate.
            bad = args.directory / f"invalid-{column}-{profile}.accdb"
            if args.generate or args.derive:
                assert not bad.exists(), "Fresh invalid-control path required"
                bad.write_bytes(malformed)
            else:
                assert bad.read_bytes() == malformed
            for sql in (f"SELECT TOP 1 [{column}] FROM Streams",
                        f"SELECT [{column}] FROM Streams WHERE ID=1"):
                failed = subprocess.run(list(map(str, [args.cli, "query", "run", sql,
                    "--database", bad, "--format", "tsv"])), capture_output=True, timeout=240)
                assert failed.returncode != 0 and not failed.stdout
                controls += 1
            output = tsv(execute(args.cli, "query", "run", f"SELECT [{other}] FROM Streams",
                                 "--database", bad, "--format", "tsv"))
            assert output[1:] == [[display(wanted)] for _, wanted in streams]
            controls += 1
            hashes[str(bad.resolve().relative_to(ROOT))] = digest(bad)
        assert digest(original) == seed_hash
        for path in (original, copy):
            hashes[str(path.resolve().relative_to(ROOT))] = digest(path)
        copies.append({"profile": profile, "seed": original.name,
                       "grammar_copy": copy.name, "payload_substitutions": len(streams) * 2})
    for name, expected_hash in hashes.items():
        assert digest(ROOT / name) == expected_hash, name
    args.report.write_text(json.dumps({
        "date": "2026-10-24", "controls": controls, "compared_cli_cells": cells,
        "streams": len(streams), "hashes": hashes, "copies": copies,
        "qualification": "Original grammar streams and explicit derived payload controls; no native validity claim",
    }, indent=2) + "\n")
    print(f"Passed {controls} grammar controls, {cells} CLI cells, {len(hashes)} unchanged hashes")


if __name__ == "__main__":
    main()
