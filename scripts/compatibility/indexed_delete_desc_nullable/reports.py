"""Original-observer and cursor predictions for literal indexed DELETE images."""
import hashlib
import struct

PAGE = 4096
ITEMS = "4974656d73"
COLUMNS = ("4964", "53636f7265", "4775617264")
ROWS = ((1, -1, None), (2, 0, 1), (3, 1, 2))
INDEXES = ("PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias")
FACTS = {
    5: ("f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b", 89, 93),
    6: ("c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae", 106, 110),
}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def u16(raw, at):
    return struct.unpack_from("<H", raw, at)[0]


def u32(raw, at):
    return struct.unpack_from("<I", raw, at)[0]


def page_line(raw, number):
    page = raw[number * PAGE:(number + 1) * PAGE]
    assert len(page) == PAGE
    return ["PAGE", str(number), str(page[0]), str(page[1]),
            str(u16(page, 2)), str(u32(page, 4)), digest(page)]


def slot_line(raw, number, slot):
    page = raw[number * PAGE:(number + 1) * PAGE]
    count = u16(page, 12)
    assert page[:2] == b"\x01\x01" and slot < count <= 256
    word = u16(page, 14 + slot * 2)
    start = word & 0x1FFF
    end = PAGE if slot == 0 else u16(page, 12 + slot * 2) & 0x1FFF
    if start < 14 + count * 2 or start > end or end > PAGE:
        return ["PHYSICAL_SLOT_UNRESOLVED", str(number), str(slot), str(word), str(start), str(end)]
    body = page[start:end]
    return ["PHYSICAL_SLOT", str(number), str(slot), str(word), str(start), str(end),
            digest(body), body.hex()]


def full(before, after, original_report, header, retained):
    expected_hash, definition, data = FACTS[header]
    assert digest(before) == expected_hash and len(before) == len(after)
    retained = tuple(retained)
    assert retained in ((0, 1, 2), (1, 2), (0, 2), (1,), ())
    assert u32(before, definition * PAGE + 16) == 3
    assert u32(after, definition * PAGE + 16) == len(retained)
    ordinal_map = {old: new for new, old in enumerate(retained)}
    output = []
    observed_rows = []
    observed_cells = set()
    pages = set()
    files = 0
    unchanged = 0
    tables = 0
    td_pages = 0
    for line in original_report.decode("utf-8").splitlines():
        fields = line.split("\t")
        tag = fields[0]
        if tag == "FILE":
            assert fields == ["FILE", str(len(before)), digest(before)]
            fields = ["FILE", str(len(after)), digest(after)]
            files += 1
        elif tag == "UNCHANGED":
            assert fields == ["UNCHANGED", digest(before)]
            fields = ["UNCHANGED", digest(after)]
            unchanged += 1
        elif tag == "PAGE":
            number = int(fields[1])
            assert number not in pages and fields == page_line(before, number)
            pages.add(number)
            fields = page_line(after, number)
        elif tag == "HEADER_RAW":
            assert fields == ["HEADER_RAW", before[:PAGE].hex()]
            fields = ["HEADER_RAW", after[:PAGE].hex()]
        elif tag == "DATA_PAGE_RAW":
            number = int(fields[1])
            assert fields == ["DATA_PAGE_RAW", str(number), before[number * PAGE:(number + 1) * PAGE].hex()]
            fields = ["DATA_PAGE_RAW", str(number), after[number * PAGE:(number + 1) * PAGE].hex()]
        elif tag in ("PHYSICAL_SLOT", "PHYSICAL_SLOT_UNRESOLVED"):
            number, slot = int(fields[1]), int(fields[2])
            assert fields == slot_line(before, number, slot)
            fields = slot_line(after, number, slot)
        elif tag == "TABLE" and fields[1] == ITEMS:
            assert fields == ["TABLE", ITEMS, "false", str(definition), "3", "3", "4"]
            fields[4] = str(len(retained))
            tables += 1
        elif tag == "TD_PAGE":
            number = int(fields[2])
            old = before[number * PAGE:(number + 1) * PAGE]
            new = after[number * PAGE:(number + 1) * PAGE]
            assert fields[3:] == [digest(old), old.hex()]
            fields[3:] = [digest(new), new.hex()]
            if fields[1] == ITEMS:
                assert number == definition
                td_pages += 1
        elif tag == "ROW" and fields[1] == ITEMS:
            old = int(fields[2])
            assert fields == ["ROW", ITEMS, str(old), str(data), str(old), "3"]
            observed_rows.append(old)
            if old not in ordinal_map:
                continue
            fields[2] = str(ordinal_map[old])
        elif tag == "CELL" and fields[1] == ITEMS:
            old = int(fields[2])
            column = COLUMNS.index(fields[3])
            value = ROWS[old][column]
            token = "N" if value is None else f"I32:{value}"
            assert fields == ["CELL", ITEMS, str(old), COLUMNS[column], token]
            assert (old, column) not in observed_cells
            observed_cells.add((old, column))
            if old not in ordinal_map:
                continue
            fields[2] = str(ordinal_map[old])
        output.append("\t".join(fields))
    assert files == unchanged == tables == td_pages == 1
    assert pages == set(range(len(before) // PAGE))
    assert observed_rows == [0, 1, 2]
    assert observed_cells == {(row, column) for row in range(3) for column in range(3)}
    return ("\n".join(output) + "\n").encode("utf-8")


def cursor(retained, index):
    assert index in INDEXES
    column = 0 if index in ("PrimaryID", "PrimaryAlias") else 2
    if column == 0:
        ordered = sorted(retained, key=lambda slot: (ROWS[slot][0], slot))
    else:
        ordered = sorted(retained, key=lambda slot: (
            ROWS[slot][2] is None,
            -ROWS[slot][2] if ROWS[slot][2] is not None else 0,
            slot,
        ))
    return "".join(
        "\t".join("NULL" if value is None else str(value) for value in ROWS[slot]) + "\n"
        for slot in ordered
    ).encode("utf-8")
