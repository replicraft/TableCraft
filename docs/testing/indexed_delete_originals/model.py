"""Original-only raw source facts reused by literal indexed deletion, never a writer oracle."""

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



def construct(before, header, recipe):
    """Reconstruct DATA tombstones and both leaves solely from original facts."""
    qualified = qualify(before, header)
    facts = qualified["facts"]
    selected = tuple(recipe["selected"])
    assert selected in ((0,), (1,), (0, 2), (0, 1, 2))
    assert len(set(selected)) == len(selected)
    retained = [slot for slot in range(3) if slot not in selected]
    assert retained == recipe["remaining"]
    assert len(retained) == recipe["rows"]
    after = bytearray(before)
    patches = []

    def patch(offset, old, new, role):
        assert len(old) == len(new) and before[offset:offset + len(old)] == old
        assert after[offset:offset + len(old)] == old
        after[offset:offset + len(new)] = new
        patches.append({"offset": offset, "before": old.hex(), "after": new.hex(), "role": role})

    td = facts["definition"] * PAGE_BYTES
    patch(td + 16, struct.pack("<I", 3), struct.pack("<I", len(retained)), "declared-live-rows")
    data = facts["data"] * PAGE_BYTES
    for slot in selected:
        at = data + 14 + slot * 2
        original_word = u16(before, at)
        assert original_word == 4081 - 15 * slot
        patch(at, struct.pack("<H", original_word),
              struct.pack("<H", original_word | 0xC000), "deleted-original-root")
    for column, root in ((0, facts["id_root"]), (2, facts["guard_root"])):
        base = root * PAGE_BYTES
        order = sorted(retained, key=lambda slot: (ROWS[slot][column], slot))
        literal = b"".join(entry(ROWS[slot][column], facts["data"], slot) for slot in order)
        assert len(literal) == 9 * len(retained)
        free = PAGE_BYTES - 480 - len(literal)
        assert free == recipe["leaf_free"]
        patch(base + 2, struct.pack("<H", 3589), struct.pack("<H", free), "leaf-free")
        old_bitmap = bytearray(453)
        for end in (9, 18, 27):
            old_bitmap[end // 8] |= 1 << (end % 8)
        new_bitmap = bytearray(453)
        for end in range(9, len(literal) + 1, 9):
            new_bitmap[end // 8] |= 1 << (end % 8)
        patch(base + 27, bytes(old_bitmap), bytes(new_bitmap), "leaf-entry-ends")
        if literal:
            patch(base + 480, before[base + 480:base + 480 + len(literal)], literal,
                  "retained-original-key-and-row")
        # Existing bytes after the shorter active prefix remain untouched,
        # including newly vacated original entries and unrelated inactive tail.
        assert after[base + 480 + len(literal):base + PAGE_BYTES] == before[
            base + 480 + len(literal):base + PAGE_BYTES]
    allowed = set()
    for value in patches:
        offset = value["offset"]
        span = set(range(offset, offset + len(bytes.fromhex(value["before"]))))
        assert not allowed.intersection(span)
        allowed.update(span)
    assert len(after) == len(before)
    assert all(old == new or at in allowed for at, (old, new) in enumerate(zip(before, after)))
    for record in qualified["records"]:
        at = record["start"]
        assert after[at:at + 15] == before[at:at + 15]
    assert after[td + 20:td + PAGE_BYTES] == before[td + 20:td + PAGE_BYTES]
    return bytes(after), {
        "header": header,
        "recipe": recipe["id"],
        "selected_original_ordinals": list(selected),
        "retained_original_slots": retained,
        "rows": [list(ROWS[slot]) for slot in retained],
        "deleted_rows": len(selected),
        "patches": patches,
        "length_unchanged": True,
        "maps_aliases_history_inactive_and_complement_unchanged": True,
        "producer_calls": 0,
    }
