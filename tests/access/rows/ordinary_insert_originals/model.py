"""Literal new ordinary row and owned-DATA placement over exact originals."""
import hashlib

PAGE = 4096
ORIGINAL_RECORD = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INSERTED_RECORD = bytes.fromhex("0300180000006e00650077000c000c000600020003")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, offset, width=2):
    return int.from_bytes(raw[offset:offset + width], "little")


def construct(before, profile):
    require(len(before) == profile["bytes"], "Complete original length")
    require(hashlib.sha256(before).hexdigest() == profile["sha256"], "Exact original SHA256")
    require(number(before, 20, 4) == profile["header"], "Original ACE16 header")
    base = profile["data"] * PAGE
    definition = profile["definition"] * PAGE
    require(before[base:base + 2] == bytes([1, 1]), "Ordinary DATA signature")
    require(number(before, base + 4, 4) == profile["definition"], "Original DATA owner")
    require(number(before, base + 2) == 4014, "Original DATA free bytes")
    require(number(before, base + 12) == 1, "Original one-root slot directory")
    require(number(before, base + 14) == 0x0fbe, "Original root word")
    require(before[base + 4030:base + PAGE] == ORIGINAL_RECORD, "Exact original row and OLE")
    require(number(before, definition + 16, 4) == 1, "Original declared count")
    require(before[base + 16:base + 18] == bytes(2), "New directory cell was free")
    require(before[base + 4009:base + 4030] == bytes(21), "New payload was unused capacity")
    require(len(INSERTED_RECORD) == 21, "Complete literal inserted row")
    require(4009 - (14 + 2 * 2) == 3991, "Independent new free-space law")

    patches = [
        (definition + 16, bytes.fromhex("01000000"), bytes.fromhex("02000000")),
        (base + 2, bytes.fromhex("ae0f"), bytes.fromhex("970f")),
        (base + 12, bytes.fromhex("0100"), bytes.fromhex("0200")),
        (base + 16, bytes(2), bytes.fromhex("a90f")),
        (base + 4009, bytes(21), INSERTED_RECORD),
    ]
    after = bytearray(before)
    allowed = set()
    proof = []
    for offset, old, new in patches:
        require(len(old) == len(new), "Fixed-length patch")
        require(before[offset:offset + len(old)] == old, "Exact original patch span")
        span = set(range(offset, offset + len(old)))
        require(not allowed.intersection(span), "Disjoint source-owned patches")
        allowed.update(span)
        after[offset:offset + len(new)] = new
        proof.append([offset, old.hex(), new.hex()])
    require(all(before[index] == after[index] for index in range(len(before)) if index not in allowed),
            "Entire complement unchanged")
    require(after[base + 4030:base + PAGE] == ORIGINAL_RECORD, "Existing readonly OLE retained")
    return bytes(after), {
        "inserted_rows": 1,
        "new_logical_ordinal": 1,
        "new_reference": f"{profile['data']}:1",
        "new_record_hex": INSERTED_RECORD.hex(),
        "patches": proof,
        "length_unchanged": True,
        "producer_calls": 0,
    }


def reader(original, profile):
    require(original.endswith("\n") and "\r" not in original, "Original report framing")
    result = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Exact original target metadata")
            cells[2] = "2"
            line = "\t".join(cells) + "\n"
            tables += 1
        result.append(line)
        if cells[:2] == ["ROW", "Sentinel"]:
            require(cells == ["ROW", "Sentinel", "0", f"{profile['data']}:0", "I32:23",
                              "T:dW5jaGFuZ2VkIHNlbnRpbmVs",
                              "X:3:1528f73e0fb885f196e5189ac3bf8c70c5eb108c0dd8313f2461917244bb2e40"],
                    "Exact old row report")
            result.append(f"ROW\tSentinel\t1\t{profile['data']}:1\tI32:24\tT:bmV3\tN\n")
            rows += 1
    require(tables == rows == 1, "One original declaration and preserved original row")
    return "".join(result)
