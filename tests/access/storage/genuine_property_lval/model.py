"""Pure original/request Marker expectations; no native writer or AFTER authority."""
import base64
import hashlib

from preserved_int32_genuine import framing

PAGE = 4096
TABLE = "Sentinel"
OLD_TEXT = "unchanged sentinel"
INLINE_OLE = bytes.fromhex("030000800000000000000000c700ff")
OLE_SHA256 = "1528f73e0fb885f196e5189ac3bf8c70c5eb108c0dd8313f2461917244bb2e40"
PROFILES = {
    5: {"bytes": 458752, "sha256": "997e16d73e6b9945a52cfefa9d8531d8a970538036225039188fb2e424071ce4",
        "definition": 107, "maps": 108, "data": 110, "bitmap": 111, "sort": "09040001"},
    6: {"bytes": 528384, "sha256": "84af525badcb4d11ecf9f8bc46aee87946aa3037792795fd91c1c37b9036d67d",
        "definition": 125, "maps": 126, "data": 127, "bitmap": 128, "sort": "09040000"},
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def map_record(raw, page, slot):
    block = raw[page * PAGE:(page + 1) * PAGE]
    require(block[:2] == b"\x01\x01" and framing.number(block, 4) == 0,
            "Original direct map container owner")
    require(framing.number(block, 12, 2) == 4, "Four original direct map rows")
    start = framing.number(block, 14 + slot * 2, 2)
    end = PAGE if slot == 0 else framing.number(block, 12 + slot * 2, 2)
    require(start & ~8191 == 0 and 0 <= start < end <= PAGE and end - start == 69,
            "Original direct 69-byte map record")
    return block[start:end]


def qualify(raw, getter, header):
    require(header in PROFILES, "Literal original profile")
    facts = PROFILES[header]
    require(len(raw) == facts["bytes"] and sha256(raw) == facts["sha256"],
            "Immutable genuine original")
    require(raw[20:24] == header.to_bytes(4, "little"), "Original header word")
    inventory = framing.sdk_inventory(getter)
    observed = inventory["tables"][TABLE]
    require((observed["page"], observed["rows"], observed["maximum"])
            == (facts["definition"], 1, 3), "Original Sentinel SDK declaration")
    layout = framing.definition(raw, observed)
    require(layout["length"] == 182 and layout["variables"] == 2
            and not layout["physical"] and not layout["logical"], "Complete original Sentinel TD")
    fields = layout["fields"]
    require([(field["name"], field["native"], field["flags"], field["extended"], field["length"])
             for field in fields] == [("ID", 4, 3, 0, 4), ("Marker", 10, 2, 0, 80), ("Bytes", 11, 2, 0, 0)],
            "Actual ordinary Text plus readonly OLE sibling declarations")
    require(fields[0]["offset"] == 0 and fields[1]["variable_index"] == 0
            and fields[2]["variable_index"] == 1, "Exact fixed/variable identities")
    marker_descriptor = fields[1]["descriptor_offset"]
    require(raw[marker_descriptor + 11:marker_descriptor + 15].hex() == facts["sort"],
            "Profile-specific original sort bytes, never normalized")
    require(layout["pairs"] == [{"number": 2, "owned": facts["maps"] << 8 | 2,
                                 "candidate": facts["maps"] << 8 | 3}], "Exact Bytes LVAL declaration")
    owned = map_record(raw, facts["maps"], 0)
    require(owned == b"\x01" + facts["bitmap"].to_bytes(4, "little") + bytes(64),
            "Original indirect ordinary map has seventeen references, sixteen absent")
    bitmap = raw[facts["bitmap"] * PAGE:(facts["bitmap"] + 1) * PAGE]
    expected_bitmap = bytearray(PAGE)
    expected_bitmap[:4] = b"\x05\x01\x00\x00"
    expected_bitmap[4 + facts["data"] // 8] = 1 << (facts["data"] % 8)
    require(bitmap == expected_bitmap, "Exact original DATA membership and bitmap storage")
    candidate = bytearray(69)
    candidate[5 + facts["data"] // 8] = 1 << (facts["data"] % 8)
    require(map_record(raw, facts["maps"], 1) == candidate, "Exact original ordinary candidate map")
    require(map_record(raw, facts["maps"], 2) == bytes(69)
            and map_record(raw, facts["maps"], 3) == bytes(69), "Readonly OLE maps remain empty inline")
    rows = framing.records(raw, layout, observed)
    require(len(rows) == 1, "One immutable Sentinel row")
    row = rows[0]
    require(row["reference"] == facts["data"] << 8 and row["start"] % PAGE == 4030
            and row["end"] - row["start"] == 66 and row["offsets"] == [6, 42, 57]
            and row["footer"] - row["start"] == 57 and row["mask_hex"] == "07", "Exact original row frame")
    record = raw[row["start"]:row["end"]]
    require(framing.integer(raw, row, fields[0]) == 23, "Original manual ID")
    require(record[6:42] == OLD_TEXT.encode("utf-16le") and record[42:57] == INLINE_OLE,
            "Original raw Marker and inline OLE definition")
    require(sha256(record[54:57]) == OLE_SHA256, "Opaque OLE value pin")
    expected_tokens = ["I32:23", "T:" + base64.b64encode(OLD_TEXT.encode()).decode(), "X:3:" + OLE_SHA256]
    require(observed["records"][0]["cells"] == expected_tokens, "Original SDK row matches literal carriers")
    properties = [line for line in inventory["records"] if line[:2] == ["PROPERTY", TABLE]]
    require(properties == [["PROPERTY", TABLE, "", "PublishToWeb", "2", "0", "U8:0"]],
            "Actual nonempty passive property map observation retained")
    require(getter.endswith("\n") and "\r" not in getter, "Exact retained SDK line framing")
    return {"header": header, "row": row, "layout": layout, "getter": getter,
            "metadata": inventory, "original_marker": OLD_TEXT}


def payload(value):
    if value == {"kind": "Null"}:
        return None
    require(isinstance(value, dict) and set(value) == {"kind", "value"}
            and value["kind"] == "Text" and isinstance(value["value"], str), "Exact Text carrier")
    encoded = value["value"].encode("utf-16le")
    require(len(encoded) // 2 <= 40 and not value["value"].startswith("\ufeff"),
            "Finite reviewed raw UTF16 replacement without encoding aliases")
    return encoded


def construct(raw, qualified, recipe):
    require(recipe["outcome"] == "candidate", "No invented AFTER for refused request")
    row = qualified["row"]
    empty = recipe.get("empty", False)
    expected = {"kind": "Text", "value": OLD_TEXT}
    updates = [] if empty else [{"ordinal": 0, "row_id": {"ID": 23},
                                "expected": recipe["expected"], "replacement": recipe["replacement"]}]
    request = {"table": TABLE, "groups": [{"family": "Variable", "column": "Marker", "updates": updates}]}
    before = raw[row["start"]:row["end"]]
    after_record = bytearray(before)
    if not empty:
        require(recipe["expected"] == expected, "Exact original request expectation")
        replacement = payload(recipe["replacement"])
        active = b"" if replacement is None else replacement
        require(len(active) + len(INLINE_OLE) <= 51, "Fitting original record capacity")
        if recipe["replacement"] != expected:
            after_record[6:6 + len(active)] = active
            after_record[6 + len(active):21 + len(active)] = before[42:57]
            end = 21 + len(active)
            for at, value in [(57, end), (59, 6 + len(active)), (61, 6)]:
                after_record[at:at + 2] = value.to_bytes(2, "little")
            after_record[65] = (before[65] & ~2) | (0 if replacement is None else 2)
    changed = int(after_record != before)
    require(changed == recipe["changed_rows"], "Literal whole-record changed count")
    result = bytearray(raw)
    result[row["start"]:row["end"]] = after_record
    require(result[:row["start"]] == raw[:row["start"]]
            and result[row["end"]:] == raw[row["end"]:], "Complete file complement retained")
    require(after_record[:6] == before[:6] and after_record[63:65] == before[63:65]
            and after_record[65] & ~2 == before[65] & ~2, "Unselected fixed/footer/mask identity")
    variable = [int.from_bytes(after_record[at:at + 2], "little") for at in [61, 59, 57]]
    require(after_record[variable[1]:variable[2]] == before[42:57], "Original inline OLE descriptor moved byte-exact")
    require(after_record[variable[2]:57] == before[variable[2]:57], "Every inactive gap byte retained")
    marker = OLD_TEXT if empty else (None if recipe["replacement"] == {"kind": "Null"} else recipe["replacement"]["value"])
    token = "N" if marker is None else "T:" + base64.b64encode(marker.encode()).decode()
    reader = []
    edits = 0
    for line in qualified["getter"].splitlines(keepends=True):
        fields = line[:-1].split("\t")
        if fields[:3] == ["ROW", TABLE, "0"]:
            data_page = PROFILES[qualified["header"]]["data"]
            require(fields[3] == f"{data_page}:0" and len(fields) == 7,
                    "Original reader root/arity")
            fields[5] = token
            line = "\t".join(fields) + "\n"
            edits += 1
        reader.append(line)
    require(edits == 1, "One SDK row authority, no learned reader fields")
    result = bytes(result)
    reader = "".join(reader)
    proof = {"source_sha256": sha256(raw), "expected_sha256": sha256(result),
             "native_request": request, "changed_rows": changed,
             "record": {"reference": row["reference"], "start": row["start"], "end": row["end"],
                        "original_offsets": [6, 42, 57], "expected_offsets": variable},
             "reader_sha256": sha256(reader.encode()), "ole_inline_sha256": sha256(INLINE_OLE),
             "complement": "every byte outside original physical record unchanged",
             "inactive_gap": "copied original bytes at stationary absolute record positions",
             "native_producer_calls": 0}
    return result, proof, reader
