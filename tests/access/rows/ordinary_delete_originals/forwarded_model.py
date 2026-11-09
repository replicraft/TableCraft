"""DELETE over independently authored EOF source; never a writer-derived image."""
import base64
import hashlib

PAGE = 4096
TERMINAL = (bytes.fromhex("030017000000") + b"a\x00" * 19
            + bytes.fromhex("030000800000000000000000c700ff3b002c000600020007"))


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, start, width=2):
    return int.from_bytes(raw[start:start + width], "little")


def construct(before, profile):
    require(len(before) == profile["bytes"] and hashlib.sha256(before).hexdigest() == profile["sha256"],
            "Frozen independent first EOF expected image")
    root = profile["data"] * PAGE
    target = profile["target"] * PAGE
    definition = profile["definition"] * PAGE
    require(number(before, 20, 4) == profile["header"], "Original format header")
    require(len(before) == target + PAGE, "The hidden target is the independently added final page")
    require(number(before, definition + 16, 4) == 1, "One live logical row")
    require(number(before, root + 12) == 255 and number(before, root + 2) == 3506
            and number(before, root + 14) == 0x4fbe, "Frozen saturated forwarded root")
    require(before[root + 4030:root + PAGE] == (profile["target"] << 8).to_bytes(4, "little") + bytes(62),
            "Complete root forwarding capsule")
    require(all(number(before, root + 14 + slot * 2) == 0x8fbe for slot in range(1, 255)),
            "All other root slots remain bounded empty tombstones")
    require(number(before, target + 12) == 1 and number(before, target + 2) == 4012
            and number(before, target + 14) == 0x8fbc
            and number(before, target + 4, 4) == profile["definition"], "EOF hidden terminal owner/framing")
    require(len(TERMINAL) == 68 and before[target + 4028:target + PAGE] == TERMINAL,
            "Literal complete terminal including readonly OLE")
    after = bytearray(before)
    after[definition + 16:definition + 20] = bytes(4)
    after[root + 14:root + 16] = bytes.fromhex("becf")
    allowed = set(range(definition + 16, definition + 20)) | set(range(root + 14, root + 16))
    require(all(before[at] == after[at] for at in range(len(before)) if at not in allowed),
            "Full complement including capsule, terminal, maps and file length is exact")
    return bytes(after), {"deleted_rows": 1, "length_unchanged": True,
                          "patches": [[definition + 16, "01000000", "00000000"],
                                      [root + 14, "be4f", "becf"]],
                          "target_page_unchanged": profile["target"], "producer_calls": 0}


def reader(original, profile):
    require(original.endswith("\n") and "\r" not in original, "Pinned SDK formatter")
    result = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Original one-row metadata")
            cells[2] = "0"
            line = "\t".join(cells) + "\n"
            tables += 1
        if cells[:2] == ["ROW", "Sentinel"]:
            require(len(cells) == 7 and cells[2:5] == ["0", f"{profile['data']}:0", "I32:23"]
                    and cells[5] == "T:" + base64.b64encode(b"a" * 19).decode(),
                    "Qualified forwarded logical root and grown text")
            rows += 1
            continue
        result.append(line)
    require(tables == rows == 1, "Exactly one removed row and changed table count")
    return "".join(result)
