"""Original-only optional Int32 source facts, independently checked before literal INSERT."""

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


def construct(before, header, request):
    """Literal same-page row/leaf append; no engine or SDK output is consulted."""
    qualified = qualify(before, header)
    facts = qualified["facts"]
    additions = request["rows"]
    assert additions in ([[4, 17, None]], [[4, 17, None], [5, 18, 1]])
    assert len(additions) == request["live_added"]
    rows = [list(row) for row in ROWS] + additions
    assert len({row[0] for row in rows}) == len(rows)
    after = bytearray(before)
    spans = []

    def patch(offset, new, role):
        old = before[offset:offset + len(new)]
        assert len(old) == len(new)
        assert after[offset:offset + len(new)] == old
        after[offset:offset + len(new)] = new
        spans.append({"offset": offset, "before": old.hex(), "after": new.hex(), "role": role})

    td = facts["definition"] * PAGE_BYTES
    data = facts["data"] * PAGE_BYTES
    assert u16(before, data + 2) == 4031
    end = 4051
    starts = []
    for ordinal, row in enumerate(additions, 3):
        assert len(row) == 3 and all(type(value) is int for value in row[:2])
        assert row[2] is None or type(row[2]) is int
        payloads = [0 if value is None else value for value in row]
        mask = 3 if row[2] is None else 7
        record = b"\x03\x00" + struct.pack("<iii", *payloads) + bytes([mask])
        assert len(record) == 15
        start = end - len(record)
        starts.append(start)
        patch(data + start, record, "new-literal-complete-row")
        patch(data + 14 + 2 * ordinal, struct.pack("<H", start), "new-physical-slot")
        end = start
    assert starts == request["record_starts"]
    total = len(rows)
    free = end - (14 + 2 * total)
    assert free == request["data_free"] and free >= 0 and total <= 255
    patch(data + 2, struct.pack("<H", free), "data-free")
    patch(data + 12, struct.pack("<H", total), "data-slot-count")
    patch(td + 16, struct.pack("<I", total), "declared-live-rows")

    for physical, column, root in ((0, 0, facts["id_root"]), (1, 2, facts["guard_root"])):
        distinct = len({row[column] for row in rows}) - len({row[column] for row in ROWS})
        assert distinct == request["distinct_added"][physical]
        patch(td + 67 + physical * 12, struct.pack("<I", 3 + distinct), "historical-distinct-key-count")
        if physical == 0:
            slots = sorted(range(total), key=lambda slot: (rows[slot][0], slot))
            assert slots == list(range(total))
        else:
            slots = sorted(range(total), key=lambda slot: (
                rows[slot][2] is None,
                -rows[slot][2] if rows[slot][2] is not None else 0,
                slot,
            ))
            assert slots == request["guard_order"]
        entries = [entry(rows[slot][column], facts["data"], slot, physical == 1) for slot in slots]
        encoded = b"".join(entries)
        assert len(encoded) == request["active_bytes"][physical]
        assert PAGE_BYTES - 480 - len(encoded) == request["leaf_free"][physical]
        base = root * PAGE_BYTES
        patch(base + 2, struct.pack("<H", request["leaf_free"][physical]), "leaf-free")
        patch(base + 27, bitmap(entries), "complete-variable-width-entry-bitmap")
        patch(base + 480, encoded, "all-original-and-new-key-row-identities")
        assert after[base + 480 + len(encoded):base + PAGE_BYTES] == before[
            base + 480 + len(encoded):base + PAGE_BYTES]

    allowed = set()
    for span in spans:
        offset = span["offset"]
        covered = set(range(offset, offset + len(bytes.fromhex(span["before"]))))
        assert not allowed.intersection(covered)
        allowed.update(covered)
    assert len(after) == len(before)
    assert all(old == new or index in allowed for index, (old, new) in enumerate(zip(before, after)))
    assert after[td + 20:td + 24] == before[td + 20:td + 24] == bytes(4)
    for record in qualified["records"]:
        offset = record["start"]
        assert after[offset:offset + 15] == before[offset:offset + 15]
    return bytes(after), {
        "header": header,
        "request": request,
        "inserted_rows": len(additions),
        "final_rows": rows,
        "guard_history": 3,
        "patches": spans,
        "complete_complement_preserved": True,
        "producer_calls": 0,
    }
