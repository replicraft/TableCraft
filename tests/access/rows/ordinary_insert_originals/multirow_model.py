"""Literal two-row placement from pinned genuine BEFORE bytes, never a writer."""
import hashlib

PAGE = 4096
ORIGINAL_RECORD = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
RECORDS = {
    "new": bytes.fromhex("0300180000006e00650077000c000c000600020003"),
    "tail": bytes.fromhex("0300190000007400610069006c000e000e000600020003"),
}
TOKENS = {
    "new": ("I32:24", "T:bmV3", "N"),
    "tail": ("I32:25", "T:dGFpbA==", "N"),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, offset, width=2):
    return int.from_bytes(raw[offset:offset + width], "little")


def construct(before, profile, order):
    require(len(before) == profile["bytes"], "Complete original length")
    require(hashlib.sha256(before).hexdigest() == profile["sha256"], "Exact original SHA256")
    require(number(before, 20, 4) == profile["header"], "Original ACE16 header")
    base = profile["data"] * PAGE
    definition = profile["definition"] * PAGE
    require(before[base:base + 2] == bytes([1, 1]), "Original DATA signature")
    require(number(before, base + 4, 4) == profile["definition"], "Original owner")
    require(number(before, base + 2) == 4014, "Original DATA free bytes")
    require(number(before, base + 12) == 1, "Original slot count")
    require(number(before, base + 14) == 4030, "Original root word")
    require(before[base + 4030:base + PAGE] == ORIGINAL_RECORD, "Exact original row/OLE")
    require(number(before, definition + 16, 4) == 1, "Original TD count")
    require(before[base + 16:base + 20] == bytes(4), "Both original unused directory cells")
    require(before[base + 3986:base + 4030] == bytes(44), "Original unused payload extent")
    require(order["id"] in ("new-tail", "tail-new"), "Closed source order")
    require(len(RECORDS["new"]) == 21 and len(RECORDS["tail"]) == 23, "Literal row widths")
    require(3986 - (14 + 3 * 2) == 3966, "Independent final free-space law")

    directory = b"".join(start.to_bytes(2, "little") for start in order["record_starts"])
    payload = bytearray(44)
    frontier = 4030
    for name, start in zip(order["rows"], order["record_starts"], strict=True):
        record = RECORDS[name]
        require(start + len(record) == frontier, "Literal request-order placement")
        payload[start - 3986:start - 3986 + len(record)] = record
        frontier = start
    require(frontier == 3986, "Complete combined payload extent")
    patches = [
        (definition + 16, bytes.fromhex("01000000"), bytes.fromhex("03000000")),
        (base + 2, bytes.fromhex("ae0f"), bytes.fromhex("7e0f")),
        (base + 12, bytes.fromhex("0100"), bytes.fromhex("0300")),
        (base + 16, bytes(4), directory),
        (base + 3986, bytes(44), bytes(payload)),
    ]
    after = bytearray(before)
    allowed = set()
    proof = []
    for offset, old, new in patches:
        require(len(old) == len(new), "Fixed-length source patch")
        require(before[offset:offset + len(old)] == old, "Exact original patch bytes")
        span = set(range(offset, offset + len(old)))
        require(not allowed.intersection(span), "Disjoint source spans")
        allowed.update(span)
        after[offset:offset + len(new)] = new
        proof.append([offset, old.hex(), new.hex()])
    require(all(before[at] == after[at] for at in range(len(before)) if at not in allowed),
            "Entire complement unchanged")
    require(after[base + 4030:base + PAGE] == ORIGINAL_RECORD, "Original OLE and row retained")
    return bytes(after), {
        "order": order["id"],
        "inserted_rows": 2,
        "new_rows": [
            {"name": name, "ordinal": slot, "reference": f"{profile['data']}:{slot}",
             "start": start, "record_hex": RECORDS[name].hex()}
            for slot, (name, start) in enumerate(zip(order["rows"], order["record_starts"], strict=True), 1)
        ],
        "patches": proof,
        "length_unchanged": True,
        "producer_calls": 0,
    }


def reader(original, profile, order):
    require(original.endswith("\n") and "\r" not in original, "Original report framing")
    result = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Original exact metadata")
            cells[2] = "3"
            line = "\t".join(cells) + "\n"
            tables += 1
        result.append(line)
        if cells[:2] == ["ROW", "Sentinel"]:
            require(cells == ["ROW", "Sentinel", "0", f"{profile['data']}:0", "I32:23",
                              "T:dW5jaGFuZ2VkIHNlbnRpbmVs",
                              "X:3:1528f73e0fb885f196e5189ac3bf8c70c5eb108c0dd8313f2461917244bb2e40"],
                    "Original exact preserved row report")
            for slot, name in enumerate(order["rows"], 1):
                result.append("\t".join([
                    "ROW", "Sentinel", str(slot), f"{profile['data']}:{slot}", *TOKENS[name],
                ]) + "\n")
            rows += 1
    require(tables == rows == 1, "One original declaration and source row")
    return "".join(result)
