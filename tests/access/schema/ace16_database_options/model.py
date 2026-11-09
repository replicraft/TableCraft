"""Independently author explicit OFF metadata over the frozen creator originals.

This is a byte oracle, not imported-file admission or a native mutation service.
It calls no TableCraft codec, SDK, database process or production constructor.
"""

import argparse
from pathlib import Path
import struct


PAGE_BYTES = 4096


def short(value):
    return struct.pack("<H", value)


def long(value):
    return struct.pack("<I", value)


def put_short(buffer, offset, value):
    buffer[offset : offset + 2] = short(value)


def put_long(buffer, offset, value):
    buffer[offset : offset + 4] = long(value)


def properties():
    names = bytearray()
    for name in ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"]:
        encoded = name.encode("utf-16le")
        names.extend(short(len(encoded)))
        names.extend(encoded)

    entries = bytearray()
    for index in range(2):
        entries.extend(short(12))
        entries.extend(bytes([1, 4]))
        entries.extend(short(index))
        entries.extend(short(4))
        entries.extend(long(0))

    result = bytearray(b"MR2\0")
    result.extend(long(6 + len(names)))
    result.extend(short(128))
    result.extend(names)
    result.extend(long(12 + len(entries)))
    result.extend(short(0))
    result.extend(long(6))
    result.extend(short(0))
    result.extend(entries)
    assert len(result) == 152
    return result


def after(before):
    assert len(before) in [9 * PAGE_BYTES, 10 * PAGE_BYTES]
    assert struct.unpack_from("<H", before, 2 * PAGE_BYTES + 41)[0] == 17
    assert struct.unpack_from("<I", before, 2 * PAGE_BYTES + 16)[0] == 8
    assert struct.unpack_from("<H", before, 4 * PAGE_BYTES + 12)[0] == 8
    new_page = len(before) // PAGE_BYTES
    result = bytearray(before + bytes(PAGE_BYTES))

    row = bytearray(32)
    put_short(row, 0, 17)
    put_long(row, 2, 0x0F000004)
    put_long(row, 6, 0x0F000002)
    put_short(row, 10, 2)
    put_long(row, 28, 0x80000000)
    row.extend("MSysDb".encode("utf-16le"))
    row.extend(long(0x40000098))
    row.extend(long(new_page << 8))
    row.extend(long(0))
    row.extend(short(56))
    for variable in reversed(range(11)):
        if variable == 0:
            offset = 32
        elif variable <= 8:
            offset = 44
        else:
            offset = 56
        row.extend(short(offset))
    row.extend(short(11))
    # Dates and owner are genuinely absent, not invented values or defaults.
    row.extend(bytes([0x8F, 0x40, 0]))
    assert len(row) == 85

    catalog = 4 * PAGE_BYTES
    low = struct.unpack_from("<H", before, catalog + 28)[0]
    start = low - len(row)
    assert start >= 32
    result[catalog + start : catalog + low] = row
    put_short(result, catalog + 30, start)
    put_short(result, catalog + 12, 9)
    put_short(result, catalog + 2, start - 32)
    put_long(result, 2 * PAGE_BYTES + 16, 9)

    for slot in [16, 17]:
        map_start = struct.unpack_from("<H", before, 3 * PAGE_BYTES + 14 + 2 * slot)[0]
        bit = 3 * PAGE_BYTES + map_start + 5 + new_page // 8
        assert result[bit] == 0
        result[bit] |= 1 << (new_page % 8)

    lval = new_page * PAGE_BYTES
    result[lval : lval + 2] = bytes([1, 1])
    put_short(result, lval + 2, 3928)
    result[lval + 4 : lval + 8] = b"LVAL"
    put_short(result, lval + 12, 1)
    put_short(result, lval + 14, 3944)
    result[lval + 3944 : lval + PAGE_BYTES] = properties()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    expected = after(args.before.read_bytes())
    with args.output.open("xb") as destination:
        destination.write(expected)


if __name__ == "__main__":
    main()
