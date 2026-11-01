"""Literal original/request SDK observations, independent of INSERT producers."""
import hashlib
import struct

PAGE = 4096
ITEMS = "4974656d73"
COLUMNS = ("4964", "53636f7265", "4775617264")
ORIGINAL_ROWS = ((1, -1, None), (2, 0, 1), (3, 1, 2))
INDEXES = ("PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias")
ADDITIONS = {
    "original": (),
    "single-null": ((4, 17, None),),
    "shared-present-batch": ((4, 17, None), (5, 18, 1)),
}
FACTS = {
    5: ("f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b", 89, 93, 91, 92),
    6: ("c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae", 106, 110, 108, 109),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def u16(raw, at):
    return struct.unpack_from("<H", raw, at)[0]


def u32(raw, at):
    return struct.unpack_from("<I", raw, at)[0]


def page_line(raw, number):
    page = raw[number * PAGE:(number + 1) * PAGE]
    require(len(page) == PAGE, "Complete observed page")
    return ["PAGE", str(number), str(page[0]), str(page[1]),
            str(u16(page, 2)), str(u32(page, 4)), digest(page)]


def slot_line(raw, number, slot):
    page = raw[number * PAGE:(number + 1) * PAGE]
    count = u16(page, 12)
    require(page[:2] == bytes([1, 1]) and slot < count <= 256, "Bounded DATA slot")
    word = u16(page, 14 + slot * 2)
    start = word & 0x1fff
    end = PAGE if slot == 0 else u16(page, 12 + slot * 2) & 0x1fff
    if start < 14 + count * 2 or start > end or end > PAGE:
        return ["PHYSICAL_SLOT_UNRESOLVED", str(number), str(slot), str(word), str(start), str(end)]
    body = page[start:end]
    return ["PHYSICAL_SLOT", str(number), str(slot), str(word), str(start), str(end),
            digest(body), body.hex()]


def full(before, after, original_report, header, recipe):
    original_hash, definition, data, _, _ = FACTS[header]
    require(digest(before) == original_hash and len(before) == len(after), "Original-bound fixed image")
    additions = ADDITIONS[recipe]
    total = len(ORIGINAL_ROWS) + len(additions)
    require(u32(before, definition * PAGE + 16) == 3, "Original table count")
    require(u32(after, definition * PAGE + 16) == total, "Literal final table count")
    require(u16(after, data * PAGE + 12) == total, "Literal final physical directory")
    output = []
    pages = set()
    physical = set()
    cells = set()
    rows = []
    files = 0
    unchanged = 0
    tables = 0
    td_pages = 0
    for line in original_report.decode("utf-8").splitlines():
        fields = line.split("\t")
        tag = fields[0]
        if tag == "FILE":
            require(fields == ["FILE", str(len(before)), digest(before)], "Original file observation")
            fields = ["FILE", str(len(after)), digest(after)]
            files += 1
        elif tag == "UNCHANGED":
            require(fields == ["UNCHANGED", digest(before)], "Original final hash observation")
            fields = ["UNCHANGED", digest(after)]
            unchanged += 1
        elif tag == "PAGE":
            number = int(fields[1])
            require(number not in pages and fields == page_line(before, number), "Original page observation")
            pages.add(number)
            fields = page_line(after, number)
        elif tag == "HEADER_RAW":
            require(fields == ["HEADER_RAW", before[:PAGE].hex()], "Original header bytes")
            fields = ["HEADER_RAW", after[:PAGE].hex()]
        elif tag == "DATA_PAGE_RAW":
            number = int(fields[1])
            require(fields == [tag, str(number), before[number * PAGE:(number + 1) * PAGE].hex()],
                    "Original complete DATA bytes")
            fields = [tag, str(number), after[number * PAGE:(number + 1) * PAGE].hex()]
        elif tag in ("PHYSICAL_SLOT", "PHYSICAL_SLOT_UNRESOLVED"):
            number, slot = int(fields[1]), int(fields[2])
            require(fields == slot_line(before, number, slot), "Original physical record")
            fields = slot_line(after, number, slot)
            if number == data:
                physical.add(slot)
        elif tag == "TABLE" and fields[1] == ITEMS:
            require(fields == ["TABLE", ITEMS, "false", str(definition), "3", "3", "4"],
                    "Original complete selected-table schema")
            fields[4] = str(total)
            tables += 1
        elif tag == "TD_PAGE":
            number = int(fields[2])
            old_page = before[number * PAGE:(number + 1) * PAGE]
            new_page = after[number * PAGE:(number + 1) * PAGE]
            require(fields[3:] == [digest(old_page), old_page.hex()], "Original definition bytes")
            fields[3:] = [digest(new_page), new_page.hex()]
            if fields[1] == ITEMS:
                require(number == definition, "Selected definition identity")
                td_pages += 1
        elif tag == "ROW" and fields[1] == ITEMS:
            ordinal = int(fields[2])
            require(fields == ["ROW", ITEMS, str(ordinal), str(data), str(ordinal), "3"],
                    "Original selected-row identity")
            rows.append(ordinal)
        elif tag == "CELL" and fields[1] == ITEMS:
            ordinal = int(fields[2])
            column = COLUMNS.index(fields[3])
            require(fields == ["CELL", ITEMS, str(ordinal), COLUMNS[column],
                               ("N" if ORIGINAL_ROWS[ordinal][column] is None else f"I32:{ORIGINAL_ROWS[ordinal][column]}")], "Original exact user cell")
            require((ordinal, column) not in cells, "No duplicate original cells")
            cells.add((ordinal, column))
        output.append("\t".join(fields))
        if tag == "PHYSICAL_SLOT" and int(fields[1]) == data and int(fields[2]) == 2:
            for slot in range(3, total):
                output.append("\t".join(slot_line(after, data, slot)))
        if tag == "CELL" and fields[1:4] == [ITEMS, "2", COLUMNS[2]]:
            for slot, values in enumerate(additions, 3):
                output.append("\t".join(["ROW", ITEMS, str(slot), str(data), str(slot), "3"]))
                for column, value in zip(COLUMNS, values, strict=True):
                    output.append("\t".join(["CELL", ITEMS, str(slot), column, ("N" if value is None else f"I32:{value}")]))
    require(files == unchanged == tables == td_pages == 1, "Complete inventory boundary observations")
    require(pages == set(range(len(before) // PAGE)), "Every original physical page observed")
    require(physical == {0, 1, 2} and rows == [0, 1, 2], "Complete original slot/row observations")
    require(cells == {(row, column) for row in range(3) for column in range(3)}, "Every original user cell")
    return ("\n".join(output) + "\n").encode("utf-8")


def metadata(header):
    _, _, _, id_root, guard_root = FACTS[header]
    definitions = [
        ("PrimaryID", 0, 0, id_root, 1, "0x89", "Id"),
        ("PrimaryAlias", 1, 0, id_root, 0, "0x89", "Id"),
        ("GroupOrder", 2, 1, guard_root, 0, "0x80", "Guard"),
        ("GroupAlias", 3, 1, guard_root, 0, "0x80", "Guard"),
    ]
    return "".join("\t".join(map(str, ("Items", *fields, ("TRUE" if fields[-1] == "Guard" else "FALSE")))) + "\n"
                   for fields in definitions).encode("utf-8")


def cursor(recipe, index):
    require(index in INDEXES, "One of four frozen aliases")
    rows = ORIGINAL_ROWS + ADDITIONS[recipe]
    component = 0 if index in ("PrimaryID", "PrimaryAlias") else 2
    if component == 0:
        slots = sorted(range(len(rows)), key=lambda slot: (rows[slot][0], slot))
    else:
        slots = sorted(range(len(rows)), key=lambda slot: (
            rows[slot][2] is None,
            -rows[slot][2] if rows[slot][2] is not None else 0,
            slot,
        ))
    return "".join(
        "\t".join("NULL" if value is None else str(value) for value in rows[slot]) + "\n"
        for slot in slots
    ).encode("utf-8")
