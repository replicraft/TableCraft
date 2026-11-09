"""Independent original/request-derived same-owned-page forwarding images."""
import base64
import hashlib

PAGE = 4096
ORIGINAL = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INLINE = bytes.fromhex("030000800000000000000000c700ff")
OLD = "unchanged sentinel"
NEW = "aaaaaaaaaaaaaaaaaaa"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, at, width=2):
    value = raw[at:at + width]
    require(len(value) == width, "Truncated original field")
    return int.from_bytes(value, "little")


def put16(raw, at, value):
    raw[at:at + 2] = value.to_bytes(2, "little")


def construct(raw, getter, profile):
    require(len(raw) == profile["bytes"] and sha256(raw) == profile["sha256"], "Frozen original")
    require(number(raw, 20, 4) == profile["header"], "Original header")
    page = profile["data"]
    base = page * PAGE
    before = raw[base:base + PAGE]
    require(before[:2] == b"\x01\x01" and number(before, 4, 4) == profile["definition"],
            "Original uniquely owned Sentinel DATA page")
    require(number(before, 2) == 4014 and number(before, 12) == 1
            and number(before, 14) == 4030 and before[4030:] == ORIGINAL,
            "Original direct slot/free geometry and full row literal")
    require(ORIGINAL[6:42] == OLD.encode("utf-16le") and ORIGINAL[42:57] == INLINE,
            "Original selected/readonly sibling boundaries")
    replacement = b"a\x00" * 19
    require(replacement == NEW.encode("utf-16le") and len(replacement) == 38,
            "Literal raw UTF16 replacement, within declared forty units")
    terminal = bytearray(ORIGINAL[:6] + replacement + INLINE)
    require(len(terminal) == 59, "Literal enlarged active data end")
    terminal.extend(bytes.fromhex("3b002c000600020007"))
    require(len(terminal) == 68 and terminal[44:59] == ORIGINAL[42:57],
            "Exact readonly inline OLE relocation and nine-byte footer")
    result = bytearray(before)
    result[3962:4030] = terminal
    put16(result, 16, 3962 | 0x8000)
    put16(result, 14, 4030 | 0x4000)
    target_reference = page << 8 | 1
    result[4030:4034] = target_reference.to_bytes(4, "little")
    result[4034:4096] = bytes(62)
    put16(result, 12, 2)
    put16(result, 2, 3944)
    require(3962 - (14 + 2 * 2) == 3944, "Exact final free extent")
    require(number(result, 14) == 0x4fbe and number(result, 16) == 0x8f7a,
            "Live forwarding root and deleted/hidden terminal words")
    require(number(result, 4030, 4) == target_reference and result[4034:] == bytes(62),
            "Exact original-root forwarding capsule")
    require(result[18:3962] == before[18:3962] and result[4:12] == before[4:12],
            "Unconsumed free bytes, owner and other DATA header bytes unchanged")
    after = raw[:base] + bytes(result) + raw[base + PAGE:]
    require(len(after) == len(raw) and after[:base] == raw[:base]
            and after[base + PAGE:] == raw[base + PAGE:], "Whole other-page complement")
    require(getter.endswith("\n") and "\r" not in getter, "Original SDK framing")
    output = []
    edits = 0
    for line in getter.splitlines(keepends=True):
        fields = line[:-1].split("\t")
        if fields[:3] == ["ROW", "Sentinel", "0"]:
            require(len(fields) == 7 and fields[3] == f"{page}:0" and fields[4] == "I32:23"
                    and fields[5] == "T:" + base64.b64encode(OLD.encode()).decode(),
                    "Independent original logical root and text")
            fields[5] = "T:" + base64.b64encode(NEW.encode()).decode()
            line = "\t".join(fields) + "\n"
            edits += 1
        output.append(line)
    require(edits == 1, "Only logical Marker cell observation changes")
    reader = "".join(output)
    proof = {"source_sha256": sha256(raw), "expected_sha256": sha256(after),
             "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
             "request": {"table": "Sentinel", "groups": [{"family": "Variable", "column": "Marker",
                 "updates": [{"ordinal": 0, "identity": 23, "expected": OLD, "replacement": NEW}]}]},
             "data_page": page, "root_reference": page << 8, "hidden_reference": target_reference,
             "root_span": [base + 4030, base + 4096],
             "hidden_span": [base + 3962, base + 4030],
             "record_bytes": 68, "offsets": [6, 44, 59], "slots": 2, "free": 3944,
             "all_other_pages_maps_properties_indexes_unchanged": True,
             "native_producer_calls": 0, "normal_growth_permission": False}
    return after, proof, reader
