"""Wrap an original native image in bounded, original stored/MSZIP CAB streams.

Geometry follows the repository's original support/deployment.rs recipe. No
third-party CAB binary, fixture or writer implementation is copied. All reads
and decoded blocks are bounded to 32 KiB; inputs and outputs must be distinct.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import zlib

BLOCK_BYTES = 32768
NAME = b"application.accdb\0"
DATA_OFFSET = 36 + 8 + 16 + len(NAME)


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(BLOCK_BYTES):
            result.update(block)
    return result.hexdigest()


def cabinet(source_path, target_path, compressed):
    size = source_path.stat().st_size
    blocks = (size + BLOCK_BYTES - 1) // BLOCK_BYTES
    if not 0 < size <= 0xFFFFFFFF or not 0 < blocks <= 0xFFFF:
        raise ValueError("Image exceeds this single-folder CAB geometry")

    header = bytearray(36)
    header[:4] = b"MSCF"
    struct.pack_into("<I", header, 16, 44)
    header[24:26] = bytes([3, 1])
    struct.pack_into("<HH", header, 26, 1, 1)
    with source_path.open("rb") as source, target_path.open("xb+") as target:
        target.write(header)
        target.write(struct.pack("<IHH", DATA_OFFSET, blocks, int(compressed)))
        target.write(struct.pack("<IIHHHH", size, 0, 0, 0, 0, 0))
        target.write(NAME)
        for _ in range(blocks):
            plain = source.read(BLOCK_BYTES)
            if not plain:
                raise ValueError("Source truncated while creating CAB")
            if compressed:
                encoder = zlib.compressobj(level=1, wbits=-15)
                stored = b"CK" + encoder.compress(plain) + encoder.flush()
            else:
                stored = plain
            if len(stored) > BLOCK_BYTES + 6144:
                raise ValueError("Encoded CAB block exceeds qualified geometry")
            target.write(struct.pack("<IHH", 0, len(stored), len(plain)))
            target.write(stored)
        if source.read(1):
            raise ValueError("Source grew while creating CAB")
        end = target.tell()
        if end > 0x7FFFFFFF:
            raise ValueError("CAB exceeds bounded signed cabinet length")
        target.seek(8)
        target.write(struct.pack("<I", end))


def inspect(path, expected_size, expected_digest):
    result = hashlib.sha256()
    expanded = 0
    with path.open("rb") as source:
        header = source.read(36)
        if len(header) != 36 or header[:4] != b"MSCF":
            raise ValueError("Invalid generated CAB header")
        physical = path.stat().st_size
        if struct.unpack_from("<I", header, 8)[0] != physical:
            raise ValueError("Generated CAB length differs")
        offset, blocks, compression = struct.unpack("<IHH", source.read(8))
        size, start, folder, date, time, attributes = struct.unpack("<IIHHHH", source.read(16))
        name = source.read(len(NAME))
        if size != expected_size or start or folder or date or time or attributes:
            raise ValueError("Generated CAB file metadata differs")
        if name != NAME or offset != source.tell() or compression not in (0, 1):
            raise ValueError("Generated CAB folder/name metadata differs")
        for _ in range(blocks):
            checksum, encoded_size, plain_size = struct.unpack("<IHH", source.read(8))
            if checksum or not 0 < encoded_size <= BLOCK_BYTES + 6144:
                raise ValueError("Generated CAB block metadata differs")
            stored = source.read(encoded_size)
            if len(stored) != encoded_size or not 0 < plain_size <= BLOCK_BYTES:
                raise ValueError("Generated CAB block truncated")
            if compression:
                if stored[:2] != b"CK":
                    raise ValueError("Generated MSZIP signature differs")
                decoder = zlib.decompressobj(wbits=-15)
                plain = decoder.decompress(stored[2:], BLOCK_BYTES + 1)
                if not decoder.eof or decoder.unused_data or decoder.unconsumed_tail:
                    raise ValueError("Generated MSZIP block is not exact")
            else:
                plain = stored
            if len(plain) != plain_size:
                raise ValueError("Generated CAB expanded block length differs")
            result.update(plain)
            expanded += len(plain)
        if source.read(1) or expanded != expected_size or result.hexdigest() != expected_digest:
            raise ValueError("Generated CAB decoded bytes differ from original")
    return {
        "bytes": physical, "sha256": digest(path), "blocks": blocks,
        "compression": compression, "expanded_bytes": expanded,
        "expanded_sha256": result.hexdigest(), "file_name": NAME[:-1].decode("ascii"),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    args.output_dir.mkdir(parents=True, exist_ok=True)
    size = args.input.stat().st_size
    before = digest(args.input)
    records = {}
    for mode, compressed in [("stored", False), ("mszip", True)]:
        target = args.output_dir / f"{args.input.stem}-{mode}.accdc"
        cabinet(args.input, target, compressed)
        records[target.name] = inspect(target, size, before)
    if digest(args.input) != before:
        raise ValueError("Original source changed")
    with args.input.open("rb") as source:
        prefix = source.read(64)
    report = {
        "source_bytes": size, "source_sha256": before,
        "source_prefix64_hex": prefix.hex(), "packages": records,
        "writer_note": "Original stored/MSZIP recipe; decoded digest is checked blockwise",
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
