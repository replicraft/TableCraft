"""Original-only raw source facts reused by literal indexed deletion, never a writer oracle."""

import hashlib
import struct

PAGE_BYTES = 4096
TABLE = "Items"
COLUMNS = ("Id", "Score", "Guard")
ROWS = ((1, -1, None), (2, 0, 1), (3, 1, 2))
SOURCES = {
    5: {
        "bytes": 389120,
        "sha256": "f8f4b039d9a692a0e1f9b0ef9ee38201f60508d654705c125275a7a169ae414b",
        "definition": 89,
        "data": 93,
        "id_root": 91,
        "guard_root": 92,
        "map_page": 90,
    },
    6: {
        "bytes": 458752,
        "sha256": "c6d5f83770a25c8b6d0f34bdaab729d4d181f37fc738c47f5db9339d895fddae",
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


def entry(value, page, slot, descending=False):
    """Independent optional Int32 atom followed by original physical ROW."""
    suffix = struct.pack(">I", (page << 8) | slot)
    if value is None:
        return (b"\xff" if descending else b"\x00") + suffix
    biased = (value & 0xFFFFFFFF) ^ 0x80000000
    if descending:
        return b"\x80" + struct.pack(">I", biased ^ 0xFFFFFFFF) + suffix
    return b"\x7f" + struct.pack(">I", biased) + suffix


def order(slots, column):
    if column == 0:
        return sorted(slots, key=lambda slot: (ROWS[slot][0], slot))
    assert column == 2
    return sorted(slots, key=lambda slot: (
        ROWS[slot][2] is None,
        -ROWS[slot][2] if ROWS[slot][2] is not None else 0,
        slot,
    ))


def bitmap(entries):
    raw = bytearray(453)
    end = 0
    for item in entries:
        end += len(item)
        raw[end // 8] |= 1 << (end % 8)
    return bytes(raw)


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
        assert td[at + 6] == (1 if column == 0 else 0) and td[at + 46] == flags
        assert u32(td, at + 34) == (facts["map_page"] << 8) | (2 + physical)
        assert u32(td, at + 38) == root
        for unused in range(1, 10):
            assert u16(td, at + 4 + unused * 3) == 0xFFFF
        page = raw[root * PAGE_BYTES:(root + 1) * PAGE_BYTES]
        assert page[:2] == b"\x04\x01"
        assert u16(page, 2) == (3589 if column == 0 else 3593)
        assert u32(page, 4) == facts["definition"]
        assert page[8:27] == bytes(19)
        original_order = [0, 1, 2] if column == 0 else [2, 1, 0]
        assert order(range(3), column) == original_order
        entries = [entry(ROWS[slot][column], facts["data"], slot, column == 2)
                   for slot in original_order]
        assert [len(item) for item in entries] == ([9, 9, 9] if column == 0 else [9, 9, 5])
        assert page[27:480] == bitmap(entries)
        literal = b"".join(entries)
        assert page[480:480 + len(literal)] == literal
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
        present = [0 if value is None else value for value in values]
        mask = 3 if slot == 0 else 7
        literal = b"\x03\x00" + struct.pack("<iii", *present) + bytes([mask])
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
        entries = [entry(ROWS[slot][column], facts["data"], slot, column == 2)
                   for slot in order(retained, column)]
        literal = b"".join(entries)
        kind = "primary" if column == 0 else "guard"
        assert len(literal) == recipe[kind + "_active_bytes"]
        free = PAGE_BYTES - 480 - len(literal)
        assert free == recipe[kind + "_free"]
        original = [entry(ROWS[slot][column], facts["data"], slot, column == 2)
                    for slot in order(range(3), column)]
        old_free = 3589 if column == 0 else 3593
        patch(base + 2, struct.pack("<H", old_free), struct.pack("<H", free), "leaf-free")
        patch(base + 27, bitmap(original), bitmap(entries), "leaf-entry-ends")
        if literal:
            patch(base + 480, before[base + 480:base + 480 + len(literal)], literal,
                  "retained-original-optional-key-and-row")
        # Never erase a newly vacated entry or any inactive tail.
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
