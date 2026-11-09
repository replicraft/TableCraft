"""Literal Name-only rename over the retained creator external-OFF source.

This model calls no TableCraft planner, native record encoder or SDK writer.
The caller supplies the already evidenced OFF original, never a rename candidate.
"""

import argparse
from pathlib import Path
import struct


PAGE_BYTES = 4096


def word(buffer, offset):
    return struct.unpack_from("<H", buffer, offset)[0]


def short(value):
    return struct.pack("<H", value)


def rename(before, name):
    encoded = name.encode("utf-16le")
    assert 0 < len(encoded) <= 128
    if name.startswith("\ufeff"):
        encoded = b"\xff\xfe\x00" + encoded
    page = before[4 * PAGE_BYTES : 5 * PAGE_BYTES]
    assert word(page, 12) == 9
    user_end = word(page, 26)
    user_start = word(page, 28)
    database_start = word(page, 30)
    original = page[user_start:user_end]
    database = page[database_start:user_start]
    assert word(original, 0) == 17
    assert word(original, len(original) - 5) == 11
    assert word(original, len(original) - 7) == 32
    old_name_end = word(original, len(original) - 9)
    assert original[32:old_name_end] == "Items".encode("utf-16le")
    footer = len(original) - 29
    assert word(original, footer) == footer
    row = bytearray(original[:32])
    row.extend(encoded)
    row.extend(original[old_name_end:footer])
    row.extend(short(len(row)))
    for variable in reversed(range(11)):
        old = word(original, len(original) - 7 - 2 * variable)
        offset = 32 if variable == 0 else old - (old_name_end - 32) + len(encoded)
        row.extend(short(offset))
    row.extend(short(11))
    row.extend(original[-3:])
    new_user = user_end - len(row)
    new_database = new_user - len(database)
    assert new_database >= 32
    result = bytearray(before)
    at = 4 * PAGE_BYTES
    result[at + new_user : at + user_end] = row
    result[at + new_database : at + new_user] = database
    result[at + 28 : at + 30] = short(new_user)
    result[at + 30 : at + 32] = short(new_database)
    result[at + 2 : at + 4] = short(new_database - 32)
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("name")
    args = parser.parse_args()
    args.after.write_bytes(rename(args.before.read_bytes(), args.name))


if __name__ == "__main__":
    main()
