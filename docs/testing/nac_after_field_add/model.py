"""Literal predicted OFF-role insertion after ADD, before SDK observation.

Uses the maintained first-party MR2 OFF body; no production native codec or
SDK serializer is imported. Current sources have one fitting catalog-column
LVAL page. This oracle deliberately refuses allocation/compaction variants.
"""

import importlib.util
from pathlib import Path
import struct


PAGE = 4096
OFFSET_MASK = 0x1FFF


def u16(data, at):
    return struct.unpack_from("<H", data, at)[0]


def u32(data, at):
    return struct.unpack_from("<I", data, at)[0]


def put16(data, at, value):
    struct.pack_into("<H", data, at, value)


def put32(data, at, value):
    struct.pack_into("<I", data, at, value)


def properties(repository):
    path = repository / "docs/testing/ace16_database_options/model.py"
    spec = importlib.util.spec_from_file_location("original_off_oracle", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return bytes(module.properties())


def catalog_row(page, slot):
    row = bytearray(32)
    put16(row, 0, 17)
    put32(row, 2, 0x0F000004)
    put32(row, 6, 0x0F000002)
    put16(row, 10, 2)
    put32(row, 28, 0x80000000)
    row.extend("MSysDb".encode("utf-16le"))
    row.extend(struct.pack("<III", 0x40000098, (page << 8) | slot, 0))
    row.extend(struct.pack("<H", 56))
    for index in reversed(range(11)):
        row.extend(struct.pack("<H", 32 if index == 0 else 44 if index <= 8 else 56))
    row.extend(struct.pack("<H", 11))
    row.extend(bytes([0x8F, 0x40, 0]))
    assert len(row) == 85
    return row


def append_dense(data, page, body, allow_flags):
    base = page * PAGE
    count = u16(data, base + 12)
    words = [u16(data, base + 14 + 2 * slot) for slot in range(count)]
    assert count > 0
    if not allow_flags:
        assert all(word == (word & OFFSET_MASK) for word in words)
    offsets = [word & OFFSET_MASK for word in words]
    assert offsets == sorted(offsets, reverse=True)
    old_low = offsets[-1]
    assert u16(data, base + 2) == old_low - (14 + 2 * count)
    start = old_low - len(body)
    directory_end = 14 + 2 * (count + 1)
    assert start >= directory_end
    assert not any(data[base + directory_end:base + start])
    data[base + start:base + old_low] = body
    put16(data, base + 14 + 2 * count, start)
    put16(data, base + 12, count + 1)
    put16(data, base + 2, start - directory_end)
    return count


def expected(before, repository):
    assert len(before) == 11 * PAGE
    assert u32(before, 2 * PAGE + 16) == 8
    assert u16(before, 2 * PAGE + 41) == 17
    assert before[10 * PAGE + 4:10 * PAGE + 8] == b"LVAL"
    for slot in (16, 17):
        start = u16(before, 3 * PAGE + 14 + 2 * slot)
        row = before[3 * PAGE + start:3 * PAGE + start + 13]
        assert row == bytes([0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0])
    result = bytearray(before)
    lval_slot = append_dense(result, 10, properties(repository), False)
    catalog_slot = append_dense(result, 4, catalog_row(10, lval_slot), True)
    put32(result, 2 * PAGE + 16, 9)
    return bytes(result), {"catalog_page": 4, "catalog_slot": catalog_slot,
                           "lval_page": 10, "lval_slot": lval_slot,
                           "allocation": "fitting existing column-owned page"}
