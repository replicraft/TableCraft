"""Literal escaped and reader-alias Text rows over unchanged genuine inputs."""
import hashlib

PAGE = 4096
ORIGINAL = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
ROWS = {
    "escaped": bytes.fromhex("030018000000fffe00fffe6e0065007700110011000600020003"),
    "unescaped-alias": bytes.fromhex("030018000000fffe6e00650077000e000e000600020003"),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, offset, width=2):
    return int.from_bytes(raw[offset:offset + width], "little")


def construct(before, profile, variant):
    require(len(before) == profile["bytes"], "Complete original length")
    require(hashlib.sha256(before).hexdigest() == profile["sha256"], "Original SHA256")
    require(number(before, 20, 4) == profile["header"], "Original format word")
    base = profile["data"] * PAGE
    definition = profile["definition"] * PAGE
    require(before[base:base + 2] == bytes([1, 1]), "DATA signature")
    require(number(before, base + 4, 4) == profile["definition"], "DATA owner")
    require(number(before, base + 2) == 4014 and number(before, base + 12) == 1,
            "Original free count and one root")
    require(number(before, base + 14) == 0x0fbe, "Original root offset")
    require(before[base + 4030:base + PAGE] == ORIGINAL, "Complete old payload including OLE")
    require(number(before, definition + 16, 4) == 1, "Original declared rows")
    record = ROWS[variant["id"]]
    start = variant["start"]
    require(record.hex() == variant["record_hex"] and len(record) == variant["bytes"],
            "Two independent literal record declarations agree")
    require(start + len(record) == 4030 and start - 18 == variant["free"],
            "New slot geometry")
    require(before[base + 16:base + 18] == bytes(2), "Unused directory word")
    require(before[base + start:base + 4030] == bytes(len(record)), "Unused original capacity")
    patches = [
        (definition + 16, bytes.fromhex("01000000"), bytes.fromhex("02000000")),
        (base + 2, bytes.fromhex("ae0f"), variant["free"].to_bytes(2, "little")),
        (base + 12, bytes.fromhex("0100"), bytes.fromhex("0200")),
        (base + 16, bytes(2), start.to_bytes(2, "little")),
        (base + start, bytes(len(record)), record),
    ]
    after = bytearray(before)
    allowed = set()
    proof = []
    for offset, old, new in patches:
        require(len(old) == len(new) and before[offset:offset + len(old)] == old,
                "Exact original patch extent")
        span = set(range(offset, offset + len(old)))
        require(not allowed.intersection(span), "Disjoint literal extents")
        allowed.update(span)
        after[offset:offset + len(new)] = new
        proof.append([offset, old.hex(), new.hex()])
    require(all(before[at] == after[at] for at in range(len(before)) if at not in allowed),
            "Entire complement preserved")
    require(after[base + 4030:base + PAGE] == ORIGINAL, "Existing readonly OLE remains exact")
    return bytes(after), {
        "kind": variant["kind"],
        "observed_text": variant["observed_text"],
        "requested_text": "\ufeffnew",
        "payload_hex": variant["payload_hex"],
        "record_hex": record.hex(),
        "patches": proof,
        "producer_calls": 0,
    }


def reader(original, profile, variant):
    require(original.endswith("\n") and "\r" not in original, "Original observer framing")
    result = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Exact original table declaration")
            cells[2] = "2"
            line = "\t".join(cells) + "\n"
            tables += 1
        result.append(line)
        if cells[:2] == ["ROW", "Sentinel"]:
            require(cells == ["ROW", "Sentinel", "0", f"{profile['data']}:0", "I32:23",
                              "T:dW5jaGFuZ2VkIHNlbnRpbmVs",
                              "X:3:1528f73e0fb885f196e5189ac3bf8c70c5eb108c0dd8313f2461917244bb2e40"],
                    "Complete original row observation")
            result.append("\t".join([
                "ROW", "Sentinel", "1", f"{profile['data']}:1", "I32:24",
                variant["sdk_text_token"], "N",
            ]) + "\n")
            rows += 1
    require(tables == rows == 1, "One original target declaration and row")
    return "".join(result)
