"""Authored pre-owned alternate DATA source and literal no-allocation AFTER."""
import base64
import hashlib

PAGE = 4096
ORIGINAL_ROW = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INLINE = bytes.fromhex("030000800000000000000000c700ff")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, at, width=2):
    value = raw[at:at + width]
    require(len(value) == width, "Truncated original field")
    return int.from_bytes(value, "little")


def set_bit(raw, start, page, present):
    at, bit = start + page // 8, 1 << (page % 8)
    raw[at] = (raw[at] & ~bit) | (bit if present else 0)


def source(original, profile):
    require(len(original) == profile["bytes"] and sha256(original) == profile["sha256"],
            "Immutable genuine original")
    old, new, maps, bitmap = [profile[key] for key in ["data", "new_data", "maps", "bitmap"]]
    require(new == len(original) // PAGE and number(original, 20, 4) == profile["header"],
            "Append exactly one SOURCE page at prior EOF")
    base = old * PAGE
    require(original[base:base + 2] == b"\x01\x01"
            and number(original, base + 4, 4) == profile["definition"]
            and number(original, base + 12) == 1 and number(original, base + 14) == 4030
            and number(original, base + 2) == 4014 and original[base + 4030:base + PAGE] == ORIGINAL_ROW,
            "Original single direct row geometry")
    require(original[PAGE:PAGE + 2] == b"\x01\x01" and number(original, PAGE + 4, 4) == 1
            and number(original, PAGE + 14) == 4027
            and original[PAGE + 4027:PAGE + 4032] == bytes(5), "Actual GLOBAL sole authority row0/base0")
    global_bits = PAGE + 4027 + 5
    require(original[global_bits + new // 8] & (1 << (new % 8)), "Original future page is GLOBAL FREE")
    require(original[maps * PAGE + 4027:(maps + 1) * PAGE]
            == b"\x01" + bitmap.to_bytes(4, "little") + bytes(64), "Original indirect owned map")
    candidate_start = maps * PAGE + 3958
    candidate = bytearray(69)
    set_bit(candidate, 5, old, True)
    require(original[candidate_start:candidate_start + 69] == candidate,
            "Original singleton inline candidate/base0 map")
    expected_bitmap = bytearray(PAGE)
    expected_bitmap[:4] = bytes([5, 1, 0, 0])
    set_bit(expected_bitmap, 4, old, True)
    require(original[bitmap * PAGE:(bitmap + 1) * PAGE] == expected_bitmap,
            "Original owned bitmap with only selected DATA member")
    empty = bytearray(PAGE)
    empty[:4] = bytes([1, 1, 242, 15])  # free4082; zero slots/directory
    empty[4:8] = profile["definition"].to_bytes(4, "little")
    before = bytearray(original + empty)
    before[base + 2:base + 4] = (3506).to_bytes(2, "little")
    before[base + 12:base + 14] = (255).to_bytes(2, "little")
    for slot in range(1, 255):
        at = base + 14 + slot * 2
        before[at:at + 2] = (0x8000 | 4030).to_bytes(2, "little")
    set_bit(before, bitmap * PAGE + 4, new, True)
    set_bit(before, candidate_start + 5, old, False)
    set_bit(before, candidate_start + 5, new, True)
    set_bit(before, global_bits, new, False)
    require(4030 - (14 + 255 * 2) == 3506 and before[base + 4030:base + PAGE] == ORIGINAL_ROW,
            "Saturated SOURCE directory preserves complete original row")
    require(number(before, profile["definition"] * PAGE + 16, 4) == 1,
            "Source TD declared logical row count remains one")
    return bytes(before)


def construct(before, original_getter, profile):
    old, new = profile["data"], profile["new_data"]
    old_base, new_base = old * PAGE, new * PAGE
    require(len(before) == (new + 1) * PAGE and number(before, old_base + 12) == 255
            and number(before, old_base + 2) == 3506, "Frozen authored pre-owned SOURCE shape")
    require(before[old_base + 4030:old_base + PAGE] == ORIGINAL_ROW,
            "Original request row still direct and unchanged")
    terminal = ORIGINAL_ROW[:6] + b"a\x00" * 19 + INLINE + bytes.fromhex("3b002c000600020007")
    require(len(terminal) == 68, "Literal enlarged record")
    after = bytearray(before)
    after[new_base + 4028:new_base + PAGE] = terminal
    after[new_base + 14:new_base + 16] = (0x8000 | 4028).to_bytes(2, "little")
    after[new_base + 12:new_base + 14] = (1).to_bytes(2, "little")
    after[new_base + 2:new_base + 4] = (4012).to_bytes(2, "little")
    after[old_base + 14:old_base + 16] = (0x4000 | 4030).to_bytes(2, "little")
    after[old_base + 4030:old_base + PAGE] = (new << 8).to_bytes(4, "little") + bytes(62)
    require(len(after) == len(before) and number(after, old_base + 12) == 255
            and number(after, old_base + 2) == 3506, "No write-time allocation, source slots/free retained")
    require(after[new_base + 16:new_base + 4028] == before[new_base + 16:new_base + 4028]
            and after[old_base + 16:old_base + 4030] == before[old_base + 16:old_base + 4030],
            "All unused extents/tombstones retained")
    for page in range(new + 1):
        if page not in [old, new]:
            require(after[page * PAGE:(page + 1) * PAGE] == before[page * PAGE:(page + 1) * PAGE],
                    "Every map, metadata, property and other page unchanged during request")
    lines = []
    edits = 0
    for line in original_getter.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(len(cells) == 7 and cells[3] == f"{old}:0" and cells[4] == "I32:23"
                    and cells[5] == "T:" + base64.b64encode(b"unchanged sentinel").decode(),
                    "Original independent row authority")
            cells[5] = "T:" + base64.b64encode(b"a" * 19).decode()
            line = "\t".join(cells) + "\n"
            edits += 1
        lines.append(line)
    require(edits == 1, "Only logical Marker cell changes")
    reader = "".join(lines)
    result = bytes(after)
    proof = {"before_sha256": sha256(before), "expected_sha256": sha256(result),
             "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
             "root_reference": old << 8, "hidden_reference": new << 8,
             "source_pages": new + 1, "after_pages": new + 1,
             "hidden_span": [new_base + 4028, new_base + PAGE],
             "root_slots": 255, "root_free": 3506, "destination_slots": 1, "destination_free": 4012,
             "maps_eof_schema_property_ole_bytes_preserved_during_write": True,
             "request": {"table": "Sentinel", "column": "Marker", "ordinal": 0,
                         "expected": "unchanged sentinel", "replacement": "a" * 19},
             "native_producer_calls": 0}
    return result, proof, reader
