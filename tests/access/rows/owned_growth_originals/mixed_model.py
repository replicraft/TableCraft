"""Independent two-root source: one cross-page and one same-page growth event."""
import base64
import hashlib

PAGE = 4096
ORIGINAL_ROW = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INLINE = bytes.fromhex("030000800000000000000000c700ff")
OLD = "unchanged sentinel"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, at, width=2):
    word = raw[at:at + width]
    require(len(word) == width, "Truncated literal field")
    return int.from_bytes(word, "little")


def outside_equal(before, after, ranges):
    require(len(before) == len(after), "Whole-image length stays fixed")
    cursor = 0
    for start, end in sorted(ranges):
        require(cursor <= start <= end <= len(before), "Disjoint bounded allowed spans")
        require(before[cursor:start] == after[cursor:start], "Bytes outside allowed spans remain exact")
        cursor = end
    require(before[cursor:] == after[cursor:], "Whole-image suffix remains exact")


def row(identity, marker):
    require(identity in (23, 24) and marker in ("a" * 19, "c" * 19), "Literal request family")
    result = b"\x03\x00" + identity.to_bytes(4, "little") + marker.encode("utf-16le") + INLINE
    result += bytes.fromhex("3b002c000600020007")
    require(len(result) == 68 and result[44:59] == INLINE, "Exact enlarged row and readonly inline descriptor")
    return result


def source(alternate, profile):
    pin = profile["before_input"]
    require(len(alternate) == pin["bytes"] and sha256(alternate) == pin["sha256"],
            "Frozen alternate authored BEFORE only")
    old = profile["data"] * PAGE
    new = profile["destination"] * PAGE
    td = profile["definition"] * PAGE
    require(len(alternate) == new + PAGE and number(alternate, 20, 4) == profile["header"],
            "Source envelope and header")
    require(number(alternate, td + 16, 4) == 1 and number(alternate, old + 12) == 255
            and number(alternate, old + 2) == 3506
            and alternate[old + 4030:old + PAGE] == ORIGINAL_ROW, "Original root and saturated page")
    for slot in range(1, 255):
        require(number(alternate, old + 14 + slot * 2) == 0x8fbe, "Literal empty deleted slot")
    require(alternate[new:new + 2] == b"\x01\x01"
            and number(alternate, new + 4, 4) == profile["definition"]
            and number(alternate, new + 12) == 0 and number(alternate, new + 2) == 4082
            and alternate[new + 14:new + PAGE] == bytes(PAGE - 14), "Already-owned empty DATA source")
    record = ORIGINAL_ROW[:2] + (24).to_bytes(4, "little") + ORIGINAL_ROW[6:]
    require(len(record) == 66 and record[42:57] == INLINE, "Second authored live row")
    before = bytearray(alternate)
    before[td + 16:td + 20] = (2).to_bytes(4, "little")
    before[new + 2:new + 4] = (4014).to_bytes(2, "little")
    before[new + 12:new + 16] = bytes.fromhex("0100be0f")
    before[new + 4030:new + PAGE] = record
    outside_equal(alternate, before, [(td + 16, td + 20), (new + 2, new + 4),
                                     (new + 12, new + 16), (new + 4030, new + PAGE)])
    return bytes(before)


def predict_reader(original, profile, after):
    old, new = profile["data"], profile["destination"]
    binary = "X:3:1528f73e0fb885f196e5189ac3bf8c70c5eb108c0dd8313f2461917244bb2e40"
    lines = []
    tables = 0
    rows = 0
    for line in original.splitlines(keepends=True):
        require(line.endswith("\n"), "Complete original getter line")
        cells = line[:-1].split("\t")
        if cells[:2] == ["TABLE", "Sentinel"]:
            require(cells == ["TABLE", "Sentinel", "1", str(profile["definition"]), "3"],
                    "Original independent table metadata")
            cells[2] = "2"
            line = "\t".join(cells) + "\n"
            tables += 1
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(cells == ["ROW", "Sentinel", "0", f"{old}:0", "I32:23",
                              "T:" + base64.b64encode(OLD.encode()).decode(), binary],
                    "Original independent row and OLE authority")
            first = "a" * 19 if after else OLD
            second = "c" * 19 if after else OLD
            cells[5] = "T:" + base64.b64encode(first.encode()).decode()
            line = "\t".join(cells) + "\n"
            line += "\t".join(["ROW", "Sentinel", "1", f"{new}:0", "I32:24",
                                "T:" + base64.b64encode(second.encode()).decode(), binary]) + "\n"
            rows += 1
        lines.append(line)
    require((tables, rows) == (1, 1), "Exactly one table/count and one original row projection")
    return "".join(lines)


def construct(before, original_reader, profile):
    old = profile["data"] * PAGE
    new = profile["destination"] * PAGE
    require(number(before, profile["definition"] * PAGE + 16, 4) == 2
            and number(before, old + 12) == 255 and number(before, old + 2) == 3506
            and number(before, new + 12) == 1 and number(before, new + 2) == 4014,
            "Frozen two-root SOURCE geometry")
    require(before[old + 4030:old + PAGE] == ORIGINAL_ROW
            and before[new + 4030:new + PAGE] == ORIGINAL_ROW[:2] + (24).to_bytes(4, "little") + ORIGINAL_ROW[6:],
            "Both original user requests are literal live roots")
    after = bytearray(before)
    after[new + 3962:new + 4030] = row(23, "a" * 19)
    after[new + 3894:new + 3962] = row(24, "c" * 19)
    after[new + 2:new + 4] = (3874).to_bytes(2, "little")
    after[new + 12:new + 20] = bytes.fromhex("0300be4f7a8f368f")
    after[old + 14:old + 16] = bytes.fromhex("be4f")
    after[old + 4030:old + PAGE] = ((profile["destination"] << 8) | 1).to_bytes(4, "little") + bytes(62)
    after[new + 4030:new + PAGE] = ((profile["destination"] << 8) | 2).to_bytes(4, "little") + bytes(62)
    outside_equal(before, after, [(old + 14, old + 16), (old + 4030, old + PAGE),
                                 (new + 2, new + 4), (new + 12, new + 20),
                                 (new + 3894, new + PAGE)])
    require(3894 - (14 + 3 * 2) == 3874 and number(after, old + 12) == 255
            and number(after, old + 2) == 3506, "Exact capacity reservation arithmetic")
    require(after[new + 3962 + 44:new + 3962 + 59] == INLINE
            and after[new + 3894 + 44:new + 3894 + 59] == INLINE, "Both readonly OLE descriptors unchanged")
    before_reader = predict_reader(original_reader, profile, False)
    after_reader = predict_reader(original_reader, profile, True)
    proof = {"before_sha256": sha256(before), "expected_sha256": sha256(after),
             "changed_rows": 2, "root_references": [profile["data"] << 8, profile["destination"] << 8],
             "hidden_references": [(profile["destination"] << 8) | 1, (profile["destination"] << 8) | 2],
             "hidden_spans": [[new + 3962, new + 4030], [new + 3894, new + 3962]],
             "old_root_slots": 255, "old_root_free": 3506, "destination_slots": 3,
             "destination_free": 3874, "schema_maps_properties_indexes_eof_unchanged_during_write": True,
             "native_producer_calls": 0}
    return bytes(after), proof, before_reader, after_reader
