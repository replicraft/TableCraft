"""Pure original-only EOF DATA allocation source and literal AFTER encoder."""
import base64
import hashlib

PAGE = 4096
ORIGINAL_ROW = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INLINE_OLE = bytes.fromhex("030000800000000000000000c700ff")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, at, width=2):
    part = raw[at:at + width]
    require(len(part) == width, "Truncated native field")
    return int.from_bytes(part, "little")


def change_bit(raw, start, page, value):
    at = start + page // 8
    mask = 1 << (page % 8)
    raw[at] = (raw[at] & ~mask) | (mask if value else 0)


def original_facts(original, profile):
    require(len(original) == profile["bytes"]
            and sha256(original) == profile["sha256"], "Pinned genuine original")
    root, maps, bitmap, eof = [profile[key] for key in ["data", "maps", "bitmap", "eof"]]
    require(eof * PAGE == len(original), "True original EOF, not preowned alternate")
    require(number(original, 20, 4) == profile["header"], "Original native header")
    base = root * PAGE
    require(original[base:base + 2] == bytes([1, 1])
            and number(original, base + 4, 4) == profile["definition"]
            and number(original, base + 12) == 1
            and number(original, base + 14) == 4030
            and number(original, base + 2) == 4014
            and original[base + 4030:base + PAGE] == ORIGINAL_ROW,
            "Actual original direct row66 and page owner/free")
    global_start = PAGE + 4027
    require(original[PAGE:PAGE + 2] == bytes([1, 1])
            and number(original, PAGE + 4, 4) == 1
            and number(original, PAGE + 14) == 4027
            and original[global_start:global_start + 5] == bytes(5),
            "Original GLOBAL1:0 inline base0 carrier")
    owned_start = maps * PAGE + 4027
    require(original[owned_start:owned_start + 69]
            == bytes([1]) + bitmap.to_bytes(4, "little") + bytes(64),
            "Actual indirect owned map and bitmap reference")
    candidate_start = maps * PAGE + 3958
    candidate = bytearray(69)
    change_bit(candidate, 5, root, True)
    require(original[candidate_start:candidate_start + 69] == candidate,
            "Actual singleton candidate map; EOF not owned/hinted")
    owned_bitmap = bytearray(PAGE)
    owned_bitmap[:4] = bytes([5, 1, 0, 0])
    change_bit(owned_bitmap, 4, root, True)
    require(original[bitmap * PAGE:(bitmap + 1) * PAGE] == owned_bitmap,
            "Original bitmap declares only old root DATA")
    global_bits = global_start + 5
    owned_bits = bitmap * PAGE + 4
    candidate_bits = candidate_start + 5
    for start, expected in [(global_bits, True), (owned_bits, False), (candidate_bits, False)]:
        require(bool(original[start + eof // 8] & (1 << (eof % 8))) == expected,
                "Original EOF is explicitly FREE1, owned0 and candidate0")
    return global_bits, owned_bits, candidate_bits


def source(original, profile):
    original_facts(original, profile)
    before = bytearray(original)
    base = profile["data"] * PAGE
    before[base + 2:base + 4] = (3506).to_bytes(2, "little")
    before[base + 12:base + 14] = (255).to_bytes(2, "little")
    for slot in range(1, 255):
        at = base + 14 + slot * 2
        before[at:at + 2] = bytes.fromhex("be8f")
    require(before[base + 4030:base + PAGE] == ORIGINAL_ROW, "Live source row preserved")
    require(len(before) == len(original), "No SOURCE append or allocation")
    for page in range(profile["eof"]):
        if page != profile["data"]:
            require(before[page * PAGE:(page + 1) * PAGE]
                    == original[page * PAGE:(page + 1) * PAGE], "All other SOURCE pages unchanged")
    return bytes(before)


def construct(original, before, original_report, profile):
    global_bits, owned_bits, candidate_bits = original_facts(original, profile)
    require(before == source(original, profile), "Only declared original source derivative")
    eof = profile["eof"]
    base = profile["data"] * PAGE
    terminal = (ORIGINAL_ROW[:6] + bytes([97, 0]) * 19 + INLINE_OLE
                + bytes.fromhex("3b002c000600020007"))
    require(len(terminal) == 68, "Literal complete enlarged record")
    page = bytearray(PAGE)
    page[:4] = bytes.fromhex("0101ac0f")
    page[4:8] = profile["definition"].to_bytes(4, "little")
    page[12:16] = bytes.fromhex("0100bc8f")
    page[4028:PAGE] = terminal
    after = bytearray(before + page)
    after[base + 14:base + 16] = bytes.fromhex("be4f")
    after[base + 4030:base + PAGE] = (eof << 8).to_bytes(4, "little") + bytes(62)
    for start, value in [(global_bits, False), (owned_bits, True), (candidate_bits, True)]:
        change_bit(after, start, eof, value)
    allowed = set(range(base + 14, base + 16)) | set(range(base + 4030, base + PAGE))
    allowed.update(start + eof // 8 for start in [global_bits, owned_bits, candidate_bits])
    for at, (old, new) in enumerate(zip(before, after)):
        require(old == new or at in allowed, "Exact whole original-byte complement")
    for start, expected in [(global_bits, False), (owned_bits, True), (candidate_bits, True)]:
        at = start + eof // 8
        require(after[at] ^ before[at] == (1 << (eof % 8)), "Exactly one declared map bit")
        require(bool(after[at] & (1 << (eof % 8))) == expected, "Map polarity")
    require(len(after) == len(before) + PAGE, "Exactly one appended DATA page")
    lines = []
    replacements = 0
    for line in original_report.splitlines(keepends=True):
        cells = line.rstrip("\n").split("\t")
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(len(cells) == 7 and cells[3] == f"{profile['data']}:0"
                    and cells[4] == "I32:23"
                    and cells[5] == "T:" + base64.b64encode(b"unchanged sentinel").decode(),
                    "Original logical row observation")
            cells[5] = "T:" + base64.b64encode(b"a" * 19).decode()
            line = "\t".join(cells) + "\n"
            replacements += 1
        lines.append(line)
    require(replacements == 1, "One literal logical Marker replacement")
    result = bytes(after)
    proof = {
        "original_sha256": sha256(original), "before_sha256": sha256(before),
        "after_sha256": sha256(result), "before_bytes": len(before), "after_bytes": len(result),
        "root_reference": profile["data"] << 8, "hidden_reference": eof << 8,
        "changed_rows": 1, "map_claims": 3, "appended_pages": 1,
        "root_slots": 255, "root_free": 3506, "target_slots": 1, "target_free": 4012,
        "root_candidate_hint_preserved": True, "native_producer_calls": 0,
    }
    return result, proof, "".join(lines)
