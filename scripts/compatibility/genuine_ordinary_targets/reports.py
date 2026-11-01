"""Pure source-only full-observer/cursor expectations; no SDK or writer oracle."""

import hashlib
import struct

PAGE_BYTES = 4096
ITEMS = "4974656d73"
COLUMNS = ("4964", "53636f7265", "4775617264")
ORIGINAL_ROWS = ((1, -1, 0), (2, 0, 1), (3, 1, 2))
LOGICAL_INDEXES = ("PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias")
INPUTS = {
    5: (
        "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
        93,
        91,
    ),
    6: (
        "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
        110,
        108,
    ),
}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def _u16(raw, offset):
    return struct.unpack_from("<H", raw, offset)[0]


def _u32(raw, offset):
    return struct.unpack_from("<I", raw, offset)[0]


def _page_line(raw, number):
    page = raw[number * PAGE_BYTES:(number + 1) * PAGE_BYTES]
    assert len(page) == PAGE_BYTES
    return [
        "PAGE", str(number), str(page[0]), str(page[1]),
        str(_u16(page, 2)), str(_u32(page, 4)), digest(page),
    ]


def _slot_line(raw, number, slot):
    page = raw[number * PAGE_BYTES:(number + 1) * PAGE_BYTES]
    count = _u16(page, 12)
    assert page[:2] == b"\x01\x01" and slot < count <= 256
    word = _u16(page, 14 + 2 * slot)
    start = word & 0x1FFF
    end = PAGE_BYTES if slot == 0 else _u16(page, 12 + 2 * slot) & 0x1FFF
    if start < 14 + count * 2 or start > end or end > PAGE_BYTES:
        return ["PHYSICAL_SLOT_UNRESOLVED", str(number), str(slot), str(word), str(start), str(end)]
    body = page[start:end]
    return [
        "PHYSICAL_SLOT", str(number), str(slot), str(word),
        str(start), str(end), digest(body), body.hex(),
    ]


def _cell(value):
    assert value is None or type(value) is int
    return "N" if value is None else f"I32:{value}"


def _validate_after(before, after, header, rows):
    source_hash, data_page, id_leaf = INPUTS[header]
    assert digest(before) == source_hash and len(before) == len(after)
    rows = tuple(tuple(row) for row in rows)
    assert all(value is None or type(value) is int for row in rows for value in row)
    assert rows in (
        ORIGINAL_ROWS,
        ((1, 17, 0), (2, 0, 1), (3, 1, 2)),
        ((1, None, 0), (2, 0, 1), (3, 1, 2)),
        ((7, -1, 0), (2, 0, 1), (3, 1, 2)),
    )
    allowed = set()
    first = data_page * PAGE_BYTES + 4081
    if rows[0][0] == 7:
        allowed.update(range(first + 2, first + 6))
        allowed.update(range(id_leaf * PAGE_BYTES + 480, id_leaf * PAGE_BYTES + 507))
        keys = b"".join(
            b"\x7f" + struct.pack(">I", value ^ 0x80000000) + struct.pack(">I", (data_page << 8) | slot)
            for value, slot in ((2, 1), (3, 2), (7, 0))
        )
        assert after[id_leaf * PAGE_BYTES + 480:id_leaf * PAGE_BYTES + 507] == keys
    elif rows[0][1] is None:
        allowed.add(first + 14)
    elif rows[0][1] == 17:
        allowed.update(range(first + 6, first + 10))
    for index, (old, new) in enumerate(zip(before, after)):
        assert old == new or index in allowed
    for slot, row in enumerate(rows):
        start = data_page * PAGE_BYTES + (4081 - 15 * slot)
        mask = after[start + 14]
        assert mask == (5 if row[1] is None else 7)
        assert after[start:start + 2] == b"\x03\x00"
        for column, value in enumerate(row):
            at = start + 2 + 4 * column
            if value is None:
                assert after[at:at + 4] == before[at:at + 4]
            else:
                assert struct.unpack_from("<i", after, at)[0] == value
    return rows


def full_observer(before, after, original_report, header, final_rows):
    """Transform ORIGINAL observations only; preserve every other semantic line."""
    rows = _validate_after(before, after, header, final_rows)
    data_page = INPUTS[header][1]
    text = original_report.decode("utf-8")
    assert text.endswith("\n")
    output = []
    pages = set()
    observed_rows = []
    observed_cells = set()
    file_count = 0
    unchanged_count = 0
    for line in text.splitlines():
        fields = line.split("\t")
        tag = fields[0]
        if tag == "FILE":
            assert fields == ["FILE", str(len(before)), digest(before)]
            fields = ["FILE", str(len(after)), digest(after)]
            file_count += 1
        elif tag == "UNCHANGED":
            assert fields == ["UNCHANGED", digest(before)]
            fields = ["UNCHANGED", digest(after)]
            unchanged_count += 1
        elif tag == "PAGE":
            number = int(fields[1])
            assert number not in pages and fields == _page_line(before, number)
            pages.add(number)
            fields = _page_line(after, number)
        elif tag == "HEADER_RAW":
            assert fields == ["HEADER_RAW", before[:PAGE_BYTES].hex()]
            fields = ["HEADER_RAW", after[:PAGE_BYTES].hex()]
        elif tag == "DATA_PAGE_RAW":
            number = int(fields[1])
            old = before[number * PAGE_BYTES:(number + 1) * PAGE_BYTES]
            new = after[number * PAGE_BYTES:(number + 1) * PAGE_BYTES]
            assert fields == ["DATA_PAGE_RAW", str(number), old.hex()]
            fields = ["DATA_PAGE_RAW", str(number), new.hex()]
        elif tag in ("PHYSICAL_SLOT", "PHYSICAL_SLOT_UNRESOLVED"):
            number, slot = int(fields[1]), int(fields[2])
            assert fields == _slot_line(before, number, slot)
            fields = _slot_line(after, number, slot)
        elif tag == "ROW" and fields[1] == ITEMS:
            ordinal = int(fields[2])
            assert fields == ["ROW", ITEMS, str(ordinal), str(data_page), str(ordinal), "3"]
            observed_rows.append(ordinal)
        elif tag == "CELL" and fields[1] == ITEMS:
            ordinal = int(fields[2])
            column = COLUMNS.index(fields[3])
            assert fields == [
                "CELL", ITEMS, str(ordinal), COLUMNS[column],
                _cell(ORIGINAL_ROWS[ordinal][column]),
            ]
            assert (ordinal, column) not in observed_cells
            observed_cells.add((ordinal, column))
            fields = ["CELL", ITEMS, str(ordinal), COLUMNS[column], _cell(rows[ordinal][column])]
        output.append("\t".join(fields))
    assert file_count == unchanged_count == 1
    assert pages == set(range(len(before) // PAGE_BYTES))
    assert observed_rows == [0, 1, 2]
    assert observed_cells == {(row, column) for row in range(3) for column in range(3)}
    return ("\n".join(output) + "\n").encode("utf-8")


def cursor(final_rows, logical_index):
    """IndexRowsOracle has no header and prints NULL, not the full-observer N."""
    assert logical_index in LOGICAL_INDEXES
    rows = tuple(tuple(row) for row in final_rows)
    assert len(rows) == 3 and all(len(row) == 3 for row in rows)
    assert all(value is None or type(value) is int for row in rows for value in row)
    column = 0 if logical_index in ("PrimaryID", "PrimaryAlias") else 2
    order = sorted(range(3), key=lambda slot: (rows[slot][column], slot))
    return ("".join(
        "\t".join("NULL" if value is None else str(value) for value in rows[slot]) + "\n"
        for slot in order
    )).encode("utf-8")
