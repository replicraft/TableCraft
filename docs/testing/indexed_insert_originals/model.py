"""Manual indexed INSERT expectations from frozen original-only raw facts."""
import hashlib
import struct

PAGE = 4096
ENTRY_START = 480
BITMAP_START = 27
BITMAP_BYTES = 453
ORIGINAL_ROWS = ((1, -1, 0), (2, 0, 1), (3, 1, 2))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def construct(before, qualified, request, facts_owner):
    header = qualified["header"]
    require(qualified == facts_owner.qualify(before, header), "Shared original interpretation")
    facts = qualified["facts"]
    require(tuple(tuple(row) for row in ORIGINAL_ROWS) == facts_owner.ROWS,
            "Frozen original cells")
    require(request["outcome"] in ["image", "original", "refuse"], "Closed source request")
    additions = request["rows"]
    for row in additions:
        require(len(row) == 3 and all(type(value) is int and -(1 << 31) <= value < (1 << 31)
                                     for value in row), "Complete exact Int32 values")

    original_keys = {row[0] for row in ORIGINAL_ROWS}
    primary = [row[0] for row in additions]
    duplicate = len(set(primary)) != len(primary) or any(value in original_keys for value in primary)
    if request["outcome"] == "refuse":
        require(duplicate, "Literal unique primary conflict, no AFTER expectation")
        return None, {"reason": "unique primary conflict", "producer_calls": 0}
    require(not duplicate, "Healthy explicit primary keys")
    if request["outcome"] == "original":
        require(additions == [] and request["live_added"] == 0
                and request["distinct_added"] == [0, 0], "Empty requests preserve original image")
        return bytes(before), {"inserted_rows": 0, "producer_calls": 0}

    require(additions and len(additions) == request["live_added"], "Explicit nonempty row count")
    rows = [list(row) for row in ORIGINAL_ROWS] + additions
    after = bytearray(before)
    allowed = []
    td = facts["definition"] * PAGE
    data = facts["data"] * PAGE
    require(struct.unpack_from("<H", before, data + 2)[0] == 4031, "Original DATA free bytes")

    end = 4051
    for offset, row in enumerate(additions):
        record = bytes([3, 0]) + struct.pack("<iii", *row) + bytes([7])
        require(len(record) == 15, "Ordinary three-field fixed record")
        start = end - 15
        after[data + start:data + end] = record
        slot = 3 + offset
        directory = data + 14 + 2 * slot
        after[directory:directory + 2] = struct.pack("<H", start)
        allowed.extend([(data + start, data + end), (directory, directory + 2)])
        end = start
    total = len(rows)
    free = end - (14 + 2 * total)
    require(free >= 0 and total <= 255, "Existing owned DATA capacity")
    after[data + 2:data + 4] = struct.pack("<H", free)
    after[data + 12:data + 14] = struct.pack("<H", total)
    after[td + 16:td + 20] = struct.pack("<I", total)
    allowed.extend([(data + 2, data + 4), (data + 12, data + 14), (td + 16, td + 20)])

    deltas = []
    for physical, column, root in [(0, 0, facts["id_root"]), (1, 2, facts["guard_root"])]:
        distinct = len({row[column] for row in rows}) - len({row[column] for row in ORIGINAL_ROWS})
        require(distinct == request["distinct_added"][physical], "Independent distinct-key delta")
        history = td + 67 + 12 * physical
        require(struct.unpack_from("<I", before, history)[0] == 3, "Original historical counter")
        after[history:history + 4] = struct.pack("<I", 3 + distinct)
        allowed.append((history, history + 4))
        deltas.append(distinct)

        entries = sorted((row[column], slot) for slot, row in enumerate(rows))
        encoded = b"".join(facts_owner.entry(value, facts["data"], slot) for value, slot in entries)
        require(len(encoded) == total * 9, "Every original/new ROW occurs once in each physical index")
        bitmap = bytearray(BITMAP_BYTES)
        for index in range(total):
            terminal = (index + 1) * 9
            bitmap[terminal // 8] |= 1 << (terminal % 8)
        base = root * PAGE
        after[base + 2:base + 4] = struct.pack("<H", PAGE - ENTRY_START - len(encoded))
        after[base + BITMAP_START:base + ENTRY_START] = bitmap
        after[base + ENTRY_START:base + ENTRY_START + len(encoded)] = encoded
        allowed.extend([(base + 2, base + 4), (base + BITMAP_START, base + ENTRY_START),
                        (base + ENTRY_START, base + ENTRY_START + len(encoded))])
        require(after[base + ENTRY_START + len(encoded):base + PAGE]
                == before[base + ENTRY_START + len(encoded):base + PAGE], "Unused leaf tail preserved")

    require(len(after) == len(before), "No file allocation")
    require(after[td + 20:td + 24] == before[td + 20:td + 24] == bytes(4), "Manual table LAST remains zero")
    require(all(before[at] == after[at] for at in range(len(before))
                if not any(start <= at < stop for start, stop in allowed)),
            "Complete complement: maps, descriptors, old records and unrelated objects")
    return bytes(after), {"header": header, "inserted_rows": len(additions),
                          "final_rows": rows, "distinct_added": deltas,
                          "changed_spans": allowed, "producer_calls": 0}
