"""Two literal root-delete patches over unchanged genuine original inputs."""
import hashlib

PAGE = 4096
RECORD = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, start, width=2):
    return int.from_bytes(raw[start:start + width], "little")


def construct(before, profile):
    require(len(before) == profile["bytes"] and hashlib.sha256(before).hexdigest() == profile["sha256"],
            "Exact genuine original")
    base = profile["data"] * PAGE
    definition = profile["definition"] * PAGE
    require(number(before, 20, 4) == profile["header"], "Original ACE16 header")
    require(before[base:base + 2] == bytes([1, 1])
            and number(before, base + 4, 4) == profile["definition"], "Original DATA owner")
    require(number(before, base + 2) == 4014 and number(before, base + 12) == 1
            and number(before, base + 14) == 0x0fbe, "Original one-root geometry")
    require(before[base + 4030:base + PAGE] == RECORD and len(RECORD) == 66,
            "Complete original row including readonly OLE")
    require(number(before, definition + 16, 4) == 1, "Original TD count")
    after = bytearray(before)
    after[definition + 16:definition + 20] = bytes(4)
    after[base + 14:base + 16] = bytes.fromhex("becf")
    allowed = set(range(definition + 16, definition + 20)) | set(range(base + 14, base + 16))
    require(all(before[at] == after[at] for at in range(len(before)) if at not in allowed),
            "Entire complement preserved, including payload, maps and properties")
    require(after[base + 4030:base + PAGE] == RECORD, "Deleted payload remains exact")
    return bytes(after), {"deleted_rows": 1, "length_unchanged": True,
                          "patches": [[definition + 16, "01000000", "00000000"],
                                      [base + 14, "be0f", "becf"]],
                          "producer_calls": 0}


def reader(original, profile):
    require(original.endswith("\n") and "\r" not in original, "Original SDK framing")
    result = []
    table_count = 0
    row_count = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Original complete table metadata")
            cells[2] = "0"
            line = "\t".join(cells) + "\n"
            table_count += 1
        if cells[:2] == ["ROW", "Sentinel"]:
            require(len(cells) == 7 and cells[2:5] == ["0", f"{profile['data']}:0", "I32:23"],
                    "Exactly the selected original logical root")
            row_count += 1
            continue
        result.append(line)
    require(table_count == row_count == 1, "One declaration and one removed row only")
    return "".join(result)
