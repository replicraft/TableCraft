"""Independent two-root SOURCE and competing already-owned page reservations."""
import base64
import hashlib

PAGE = 4096
OLD = "unchanged sentinel"
INLINE = bytes.fromhex("030000800000000000000000c700ff")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, at, width=2):
    return int.from_bytes(raw[at:at + width], "little")


def put16(raw, at, value):
    raw[at:at + 2] = value.to_bytes(2, "little")


def set_bit(raw, start, page):
    raw[start + page // 8] |= 1 << (page % 8)


def empty_page(definition):
    page = bytearray(PAGE)
    page[:4] = bytes([1, 1, 242, 15])
    page[4:8] = definition.to_bytes(4, "little")
    return page


def source(parent, profile, no_capacity=False):
    require(len(parent) == profile["bytes"] and sha256(parent) == profile["sha256"],
            "Frozen authored alternate parent BEFORE")
    root, first, second = [profile[key] for key in ["root", "first", "second"]]
    rb, fb, sb = [page * PAGE for page in [root, first, second]]
    require(second == len(parent) // PAGE and first + 1 == second, "Append one SOURCE page")
    require(number(parent, rb + 12) == 255 and number(parent, rb + 2) == 3506
            and number(parent, rb + 14) == 4030, "Actual saturated single-root parent")
    row = parent[rb + 4030:rb + PAGE]
    require(len(row) == 66 and row[:6] == bytes([3, 0, 23, 0, 0, 0])
            and row[6:42] == OLD.encode("utf-16le") and row[42:57] == INLINE
            and row[57:] == bytes.fromhex("39002a000600020007"), "Complete original raw row")
    require(all(number(parent, rb + 14 + slot * 2) == 0x8fbe for slot in range(1, 255)),
            "Parent deleted-only empty directory")
    require(parent[fb:fb + PAGE] == empty_page(profile["definition"]), "Actual pre-owned empty first destination")
    before = bytearray(parent + empty_page(profile["definition"]))
    second_row = bytearray(row)
    second_row[2:6] = (24).to_bytes(4, "little")
    before[rb + 3964:rb + 4030] = second_row
    put16(before, rb + 16, 3964)
    for slot in range(2, 255):
        put16(before, rb + 14 + slot * 2, 0x8000 | 3964)
    put16(before, rb + 2, 3440)
    before[profile["definition"] * PAGE + 16:profile["definition"] * PAGE + 20] = (2).to_bytes(4, "little")
    put16(before, fb + 12, 254)
    put16(before, fb + 2, 3574)
    for slot in range(254):
        put16(before, fb + 14 + slot * 2, 0x9000)
    bitmap_bits = profile["bitmap"] * PAGE + 4
    candidate_bits = profile["maps"] * PAGE + 3958 + 5
    global_bits = PAGE + 4027 + 5
    require(parent[global_bits + second // 8] & (1 << (second % 8)), "New second SOURCE page is represented future FREE")
    set_bit(before, bitmap_bits, second)
    set_bit(before, candidate_bits, second)
    before[global_bits + second // 8] &= ~(1 << (second % 8))
    if no_capacity:
        put16(before, sb + 12, 255)
        put16(before, sb + 2, 3572)
        for slot in range(255):
            put16(before, sb + 14 + slot * 2, 0x9000)
    require(before[rb + 4030:rb + PAGE] == row
            and before[rb + 3964:rb + 4030] == second_row, "Both complete live SOURCE rows")
    require(3964 - (14 + 255 * 2) == 3440 and PAGE - (14 + 254 * 2) == 3574,
            "Independent directory/free arithmetic")
    return bytes(before)


def reader(original, profile, after):
    require(original.endswith("\n") and "\r" not in original, "Independent getter line framing")
    lines = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Independent parent table declaration")
            cells[2] = "2"
            line = "\t".join(cells) + "\n"
            tables += 1
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(len(cells) == 7 and cells[3] == f"{profile['root']}:0"
                    and cells[4] == "I32:23" and cells[5] == "T:" + base64.b64encode(OLD.encode()).decode(),
                    "Independent original root identity/value")
            first = list(cells)
            second = list(cells)
            second[2:5] = ["1", f"{profile['root']}:1", "I32:24"]
            if after:
                first[5] = "T:" + base64.b64encode(b"a" * 19).decode()
                second[5] = "T:" + base64.b64encode(b"c" * 19).decode()
            line = "\t".join(first) + "\n" + "\t".join(second) + "\n"
            rows += 1
        lines.append(line)
    require(tables == rows == 1, "Exactly one changed declaration and one authored additional row")
    return "".join(lines)


def construct(before, profile):
    rb, fb, sb = [profile[key] * PAGE for key in ["root", "first", "second"]]
    require(number(before, rb + 12) == 255 and number(before, rb + 2) == 3440
            and number(before, fb + 12) == 254 and number(before, fb + 2) == 3574
            and number(before, sb + 12) == 0 and number(before, sb + 2) == 4082,
            "Two eligible reservations exist in the positive SOURCE")
    after = bytearray(before)
    for ordinal, identity, source_start, target, slot, letter in [
        (0, 23, 4030, profile["first"], 254, b"a"),
        (1, 24, 3964, profile["second"], 0, b"c"),
    ]:
        original = before[rb + source_start:rb + source_start + 66]
        require(number(original, 2, 4) == identity and original[6:42] == OLD.encode("utf-16le"),
                "Original request expected values in native root order")
        terminal = original[:6] + (letter + b"\x00") * 19 + original[42:57] + bytes.fromhex("3b002c000600020007")
        require(len(terminal) == 68 and terminal[44:59] == INLINE, "Literal grown record/readonly OLE")
        tb = target * PAGE
        after[tb + 4028:tb + PAGE] = terminal
        put16(after, tb + 14 + slot * 2, 0x8fbc)
        put16(after, tb + 12, slot + 1)
        put16(after, tb + 2, 3504 if slot == 254 else 4012)
        after[rb + source_start:rb + source_start + 66] = ((target << 8) | slot).to_bytes(4, "little") + bytes(62)
        put16(after, rb + 14 + ordinal * 2, source_start | 0x4000)
    require(after[rb + 2:rb + 4] == before[rb + 2:rb + 4]
            and after[rb + 18:rb + 3964] == before[rb + 18:rb + 3964], "Root free extent/tombstones retained")
    for page in range(len(before) // PAGE):
        if page not in [profile["root"], profile["first"], profile["second"]]:
            require(after[page * PAGE:(page + 1) * PAGE] == before[page * PAGE:(page + 1) * PAGE],
                    "Maps/schema/properties/other data unchanged during request")
    result = bytes(after)
    return result, {"before_sha256": sha256(before), "expected_sha256": sha256(result),
                    "changed_rows": 2, "ordered_reservations": [
                        {"ordinal": 0, "target": profile["first"], "slot": 254, "identity": 23},
                        {"ordinal": 1, "target": profile["second"], "slot": 0, "identity": 24}],
                    "maps_eof_unchanged": True, "native_producer_calls": 0}
