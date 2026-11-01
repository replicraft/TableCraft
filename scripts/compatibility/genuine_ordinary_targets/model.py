"""Independent literal edits of two immutable genuine originals; never a writer oracle."""

import hashlib
import struct

PAGE_BYTES = 4096
TABLE = "Items"
COLUMNS = ("Id", "Score", "Guard")
ROWS = ((1, -1, 0), (2, 0, 1), (3, 1, 2))
SOURCES = {
    5: {
        "bytes": 405504,
        "sha256": "72e7ba5480d95d0be0d4ea6f2c75b015062970599da3fde620cc9f13c60b028d",
        "definition": 89,
        "data": 93,
        "id_root": 91,
        "guard_root": 92,
        "map_page": 90,
    },
    6: {
        "bytes": 475136,
        "sha256": "5cfbf6d4b6224b989f00f0abd63ec362846ff60240a91b058e1442c3010305c5",
        "definition": 106,
        "data": 110,
        "id_root": 108,
        "guard_root": 109,
        "map_page": 107,
    },
}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def u16(raw, offset):
    return struct.unpack_from("<H", raw, offset)[0]


def u32(raw, offset):
    return struct.unpack_from("<I", raw, offset)[0]


def entry(value, page, slot):
    return (
        b"\x7f"
        + struct.pack(">I", (value & 0xFFFFFFFF) ^ 0x80000000)
        + struct.pack(">I", (page << 8) | slot)
    )


def qualify(raw, header):
    """Check actual raw bindings against frozen original-only literal facts."""
    facts = SOURCES[header]
    assert len(raw) == facts["bytes"] and digest(raw) == facts["sha256"]
    assert raw[20:24] == struct.pack("<I", header)
    td_base = facts["definition"] * PAGE_BYTES
    td = raw[td_base:td_base + PAGE_BYTES]
    assert td[:2] == b"\x02\x01"
    assert u32(td, 8) == 500 and u32(td, 16) == 3
    assert u32(td, 20) == 0 and u16(td, 41) == 3 and u16(td, 43) == 0
    assert u16(td, 45) == 3 and u32(td, 47) == 4 and u32(td, 51) == 2
    assert u32(td, 55) == facts["map_page"] << 8
    assert u32(td, 59) == (facts["map_page"] << 8) | 1
    assert u32(td, 67) == 3 and u32(td, 79) == 3
    names_at = 87 + 3 * 25
    for position, name in enumerate(COLUMNS):
        at = 87 + position * 25
        assert td[at] == 4 and u32(td, at + 1) == 1625
        assert u16(td, at + 5) == position and u16(td, at + 9) == position
        assert td[at + 15:at + 17] == b"\x03\x00"
        assert u16(td, at + 21) == position * 4 and u16(td, at + 23) == 4
        encoded = name.encode("utf-16le")
        assert u16(td, names_at) == len(encoded)
        assert td[names_at + 2:names_at + 2 + len(encoded)] == encoded
        names_at += 2 + len(encoded)
    assert names_at == 192
    for physical, column, root, flags in (
        (0, 0, facts["id_root"], 0x89),
        (1, 2, facts["guard_root"], 0x80),
    ):
        at = names_at + physical * 52
        assert u32(td, at) == 1923 and u16(td, at + 4) == column
        assert td[at + 6] == 1 and td[at + 46] == flags
        assert u32(td, at + 34) == (facts["map_page"] << 8) | (2 + physical)
        assert u32(td, at + 38) == root
        for unused in range(1, 10):
            assert u16(td, at + 4 + unused * 3) == 0xFFFF
        page = raw[root * PAGE_BYTES:(root + 1) * PAGE_BYTES]
        assert page[:2] == b"\x04\x01" and u16(page, 2) == 3589
        assert u32(page, 4) == facts["definition"]
        assert page[8:27] == bytes(19)
        bitmap = bytearray(453)
        for end in (9, 18, 27):
            bitmap[end // 8] |= 1 << (end % 8)
        assert page[27:480] == bytes(bitmap)
        literal = b"".join(entry(row[column], facts["data"], slot) for slot, row in enumerate(ROWS))
        assert page[480:507] == literal
    logical_at = names_at + 2 * 52
    for logical, physical, kind in ((0, 0, 1), (1, 0, 0), (2, 1, 0), (3, 1, 0)):
        at = logical_at + logical * 28
        assert u32(td, at) == 1625 and u32(td, at + 4) == logical
        assert u32(td, at + 8) == physical and td[at + 23] == kind
    names_at = logical_at + 4 * 28
    for name in ("PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"):
        encoded = name.encode("utf-16le")
        assert u16(td, names_at) == len(encoded)
        assert td[names_at + 2:names_at + 2 + len(encoded)] == encoded
        names_at += 2 + len(encoded)
    assert names_at == 498 and td[498:500] == b"\xff\xff"
    data_base = facts["data"] * PAGE_BYTES
    data = raw[data_base:data_base + PAGE_BYTES]
    assert data[:2] == b"\x01\x01" and u32(data, 4) == facts["definition"]
    assert u16(data, 12) == 3
    records = []
    for slot, (start, values) in enumerate(zip((4081, 4066, 4051), ROWS)):
        assert u16(data, 14 + 2 * slot) == start
        literal = b"\x03\x00" + struct.pack("<iii", *values) + b"\x07"
        assert data[start:start + 15] == literal
        records.append({"page": facts["data"], "slot": slot, "start": data_base + start, "length": 15})
    return {"header": header, "facts": facts.copy(), "records": records}


def _expected(value):
    if value == {"type": "Null"}:
        return None
    assert value["type"] == "Int32" and set(value) == {"type", "value"}
    assert type(value["value"]) is int and -(1 << 31) <= value["value"] < (1 << 31)
    return value["value"]


def construct(raw, qualified, case):
    """Return a complete source-derived image and a declared patch complement."""
    header = qualified["header"]
    assert qualified == qualify(raw, header)
    assert case["header"] == header and case["table"] == TABLE
    assert case["expected_outcome"]["kind"] == "candidate"
    assert case["column"] in ("Id", "Score")
    position = COLUMNS.index(case["column"])
    records = qualified["records"]
    after = bytearray(raw)
    rows = [list(row) for row in ROWS]
    patches = []
    changed = set()
    for update in case["updates"]:
        slot = update["row"]
        assert slot == 0
        before = _expected(update["expected"])
        replacement = _expected(update["replacement"])
        assert before == ROWS[slot][position]
        if before == replacement:
            continue
        record = records[slot]
        if replacement is not None:
            at = record["start"] + 2 + position * 4
            payload = struct.pack("<i", replacement)
            after[at:at + 4] = payload
            patches.append({
                "offset": at,
                "before": raw[at:at + 4].hex(),
                "after": payload.hex(),
                "role": "fixed-payload",
            })
        else:
            assert position == 1
            at = record["start"] + 14
            assert raw[at] == 7
            after[at] &= ~(1 << position)
            patches.append({
                "offset": at,
                "before": raw[at:at + 1].hex(),
                "after": bytes(after[at:at + 1]).hex(),
                "role": "presence-mask",
            })
        rows[slot][position] = replacement
        changed.add(slot)
    if position == 0 and changed:
        assert case["updates"] == [{
            "row": 0,
            "expected": {"type": "Int32", "value": 1},
            "replacement": {"type": "Int32", "value": 7},
        }]
        facts = qualified["facts"]
        literal = b"".join(entry(value, facts["data"], slot) for value, slot in ((2, 1), (3, 2), (7, 0)))
        assert len(literal) == 27
        at = facts["id_root"] * PAGE_BYTES + 480
        after[at:at + 27] = literal
        patches.append({
            "offset": at,
            "before": raw[at:at + 27].hex(),
            "after": literal.hex(),
            "role": "primary-index-active-entries",
        })
    after = bytes(after)
    allowed = set()
    for patch in patches:
        at = patch["offset"]
        before = bytes.fromhex(patch["before"])
        replacement = bytes.fromhex(patch["after"])
        assert len(before) == len(replacement) and raw[at:at + len(before)] == before
        assert after[at:at + len(replacement)] == replacement
        allowed.update(range(at, at + len(before)))
    assert len(after) == len(raw)
    assert all(a == b or index in allowed for index, (a, b) in enumerate(zip(raw, after)))
    assert len(changed) == case["expected_outcome"]["changed_rows"]
    return after, {
        "id": case["id"],
        "before_sha256": digest(raw),
        "after_sha256": digest(after),
        "rows": rows,
        "changed_rows": len(changed),
        "patches": patches,
        "outside_declared_patches_equal": True,
    }
