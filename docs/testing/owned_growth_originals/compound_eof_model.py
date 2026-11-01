"""Literal owned-slot plus true EOF source expectation; no writer imports."""
import base64
import hashlib

PAGE = 4096
OLD = "unchanged sentinel"
INLINE = bytes.fromhex("030000800000000000000000c700ff")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, at, width=2):
    return int.from_bytes(raw[at:at + width], "little")


def put16(raw, at, value):
    raw[at:at + 2] = value.to_bytes(2, "little")


def construct(before, profile):
    require(len(before) == profile["bytes"] and hashlib.sha256(before).hexdigest() == profile["sha256"],
            "Exact frozen no-capacity BEFORE")
    root, first, second, eof = [profile[key] for key in ["root", "first", "second", "eof"]]
    rb, fb, sb = [page * PAGE for page in [root, first, second]]
    require(len(before) == eof * PAGE and first + 1 == second and second + 1 == eof,
            "One existing owned slot then true EOF")
    require(number(before, rb + 12) == 255 and number(before, rb + 2) == 3440,
            "Two live roots within saturated directory")
    require(number(before, fb + 12) == 254 and number(before, fb + 2) == 3574,
            "First existing destination has exactly one directory slot")
    require(number(before, sb + 12) == 255 and number(before, sb + 2) == 3572,
            "Second existing destination has no directory slot")
    require(all(number(before, fb + 14 + slot * 2) == 0x9000 for slot in range(254)),
            "All first destination slots are bounded empty deleted spans")
    require(all(number(before, sb + 14 + slot * 2) == 0x9000 for slot in range(255)),
            "All second destination slots are bounded empty deleted spans")
    require(number(before, profile["definition"] * PAGE + 16, 4) == 2,
            "Original cardinality retained")

    new_page = bytearray(PAGE)
    new_page[:4] = bytes([1, 1, 242, 15])
    new_page[4:8] = profile["definition"].to_bytes(4, "little")
    after = bytearray(before + new_page)
    changed = set()
    for ordinal, identity, source_start, target, slot, letter in [
        (0, 23, 4030, first, 254, b"a"),
        (1, 24, 3964, eof, 0, b"c"),
    ]:
        original = before[rb + source_start:rb + source_start + 66]
        require(number(before, rb + 14 + ordinal * 2) == source_start,
                "Original logical roots are present ordinary records")
        require(original[:2] == bytes([3, 0]) and number(original, 2, 4) == identity
                and original[6:42] == OLD.encode("utf-16le") and original[42:57] == INLINE
                and original[57:] == bytes.fromhex("39002a000600020007"),
                "Complete original record and readonly OLE")
        terminal = original[:6] + (letter + b"\x00") * 19 + INLINE + bytes.fromhex("3b002c000600020007")
        require(len(terminal) == 68, "Exact terminal framing")
        tb = target * PAGE
        after[tb + 4028:tb + PAGE] = terminal
        put16(after, tb + 14 + slot * 2, 0x8fbc)
        put16(after, tb + 12, slot + 1)
        put16(after, tb + 2, 3504 if slot == 254 else 4012)
        after[rb + source_start:rb + source_start + 66] = ((target << 8) | slot).to_bytes(4, "little") + bytes(62)
        put16(after, rb + 14 + ordinal * 2, source_start | 0x4000)
        for start, end in [(rb + source_start, rb + source_start + 66),
                           (rb + 14 + ordinal * 2, rb + 16 + ordinal * 2)]:
            changed.update(range(start, end))
        if target == first:
            for start, end in [(tb + 4028, tb + PAGE), (tb + 522, tb + 524),
                               (tb + 12, tb + 14), (tb + 2, tb + 4)]:
                changed.update(range(start, end))

    claims = []
    for role, start, old, new in [
        ("global_free", PAGE + 4027 + 5, 1, 0),
        ("selected_owned", profile["bitmap"] * PAGE + 4, 0, 1),
        ("selected_candidate", profile["maps"] * PAGE + 3958 + 5, 0, 1),
    ]:
        at = start + eof // 8
        mask = 1 << (eof % 8)
        require(bool(before[at] & mask) == bool(old), "Represented original EOF cell: " + role)
        after[at] ^= mask
        require(bool(after[at] & mask) == bool(new), "Literal new EOF cell: " + role)
        claims.append({"role": role, "offset": at, "mask": mask, "old": old, "new": new})
        changed.add(at)
    require(len({claim["offset"] for claim in claims}) == 3, "Three exclusive storage bytes")
    require(after[sb:sb + PAGE] == before[sb:sb + PAGE], "Full second destination remains unchanged")
    require(all(before[at] == after[at] for at in range(len(before)) if at not in changed),
            "Every old byte outside literal row/directory/header/three-cell spans is preserved")
    return bytes(after), {"changed_rows": 2, "length_delta": PAGE, "claims": claims,
                          "reservations": [[0, first, 254], [1, eof, 0]],
                          "old_second_page_unchanged": True, "producer_calls": 0}


def reader(before_report):
    require(before_report.endswith("\n") and "\r" not in before_report, "SDK line framing")
    result = before_report
    for ordinal, letter in [(0, b"a"), (1, b"c")]:
        matches = [line for line in result.splitlines(keepends=True)
                   if line.startswith(f"ROW\tSentinel\t{ordinal}\t")]
        require(len(matches) == 1, "One original SDK physical root row")
        line = matches[0]
        cells = line[:-1].split("\t")
        require(len(cells) == 7 and cells[4] == f"I32:{23 + ordinal}"
                and cells[5] == "T:" + base64.b64encode(OLD.encode()).decode(),
                "Original independent logical cells")
        cells[5] = "T:" + base64.b64encode(letter * 19).decode()
        result = result.replace(line, "\t".join(cells) + "\n", 1)
    return result
