"""Literal original-only unindexed Marker overlays; never an index producer."""
import hashlib

PAGE = 4096


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def number(raw, start, width=4):
    value = raw[start:start + width]
    require(len(value) == width, "Truncated original field")
    return int.from_bytes(value, "little")


def qualify(raw, getter, profile):
    require(len(raw) == profile["bytes"] and sha256(raw) == profile["sha256"],
            "Frozen original bytes")
    require(number(raw, 20) == profile["header"], "Original ACE16 header")
    require(getter.endswith("\n") and "\r" not in getter, "Retained getter line framing")
    lines = [line.split("\t") for line in getter.splitlines()]
    facts = {}
    for name, observed in profile["tables"].items():
        definition = observed["definition"]
        block = raw[definition * PAGE:(definition + 1) * PAGE]
        require(block[:2] == b"\x02\x01" and number(block, 4) == 0,
                "Single-page original definition")
        require(number(block, 16) == observed["rows"] and number(block, 41, 2) == 3
                and number(block, 43, 2) == 0 and number(block, 45, 2) == 3,
                "Actual fixed three-column source")
        require(number(block, 47) == 4 and number(block, 51) == 3,
                "Four logical aliases and three physical indexes retained")
        descriptors = 63 + 12 * 3
        width = observed["width"]
        for ordinal, native, offset, size in [(0, 4, 0, 4),
                                              (1, observed["native"], 4, width),
                                              (2, 4, 4 + width, 4)]:
            at = descriptors + 25 * ordinal
            require(block[at] == native and number(block, at + 5, 2) == ordinal
                    and block[at + 15:at + 17] == b"\x03\x00"
                    and number(block, at + 21, 2) == offset
                    and number(block, at + 23, 2) == size,
                    "Actual ordinary native fixed descriptors")
        at = descriptors + 75
        for column in ["ID", "KeyValue", "Marker"]:
            length = number(block, at, 2)
            at += 2
            require(block[at:at + length] == column.encode("utf-16le"), "Original field names")
            at += length
        tables = [line for line in lines if line[:2] == ["TABLE", name]]
        require(tables == [["TABLE", name, str(observed["rows"]), str(definition), "3", "3"]],
                "Original independent full table observation")
        row = [line for line in lines if line[:3] == ["ROW", name, "1"]]
        require(len(row) == 1 and len(row[0]) == 8
                and row[0][3:6] == [str(observed["data"]), "0", "Integer:1"]
                and row[0][-1] == "Integer:17", "Original selected row/Marker")
        expected_key = {2: "Byte:0", 3: "Short:0", 19: "Long:0", 5: "BigDecimal:0.0000"}[observed["native"]]
        require(row[0][6] == expected_key, "Original typed numeric sibling")
        raw_rows = [line for line in lines if line[:3] == ["RAWROW", name, "1"]]
        require(raw_rows == [["RAWROW", name, "1", observed["record_hex"]]], "Original independent row bytes")
        data = observed["data"]
        page = raw[data * PAGE:(data + 1) * PAGE]
        require(page[:2] == b"\x01\x01" and number(page, 4) == definition,
                "Original selected DATA owner")
        start = number(page, 14, 2)
        require(start == PAGE - observed["record_bytes"], "Direct first slot, full record")
        record = raw[data * PAGE + start:(data + 1) * PAGE]
        require(record.hex() == observed["record_hex"], "Raw original row equals literal SDK row")
        require(record[-1] == 7 and number(record, observed["marker_record_offset"]) == 17,
                "Original present Marker value and all sibling mask bits")
        indexes = [line for line in lines if line[:2] == ["INDEX_COLUMN", name]]
        require(indexes == [["INDEX_COLUMN", name, "PrimaryID", "ID", "true"],
                            ["INDEX_COLUMN", name, "Ascending", "KeyValue", "true"],
                            ["INDEX_COLUMN", name, "Descending", "KeyValue", "false"],
                            ["INDEX_COLUMN", name, "Alias", "KeyValue", "true"]],
                "Independent index components exclude Marker")
        facts[name] = {"absolute": data * PAGE + start + observed["marker_record_offset"],
                       "record": record, "row": row[0]}
    return facts


def construct(raw, getter, profile, qualified, table):
    require(table in profile["tables"], "Original request table")
    facts = qualified[table]
    absolute = facts["absolute"]
    require(raw[absolute:absolute + 4] == b"\x11\x00\x00\x00", "Original request expected Int32")
    after = raw[:absolute] + b"\x2a\x00\x00\x00" + raw[absolute + 4:]
    require(len(after) == len(raw) and after[:absolute] == raw[:absolute]
            and after[absolute + 4:] == raw[absolute + 4:], "Complete four-byte complement")
    record = bytearray(facts["record"])
    record_at = profile["tables"][table]["marker_record_offset"]
    record[record_at:record_at + 4] = b"\x2a\x00\x00\x00"
    result = []
    edits = 0
    for line in getter.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:3] == ["ROW", table, "1"]:
            require(cells == facts["row"], "Exact original SDK row")
            cells[7] = "Integer:42"
            edits += 1
        elif cells[:3] == ["RAWROW", table, "1"]:
            cells[3] = record.hex()
            edits += 1
        result.append("\t".join(cells) + "\n")
    require(edits == 2, "Only selected value and raw-row observation change")
    reader = "".join(result)
    proof = {"source_sha256": sha256(raw), "expected_sha256": sha256(after),
             "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
             "request": {"table": table, "column": "Marker", "updates": [
                 {"ordinal": 0, "identity": 1, "expected": 17, "replacement": 42}]},
             "range": [absolute, absolute + 4], "preserved": "all other bytes including every INDEX page",
             "native_producer_calls": 0}
    return after, proof, reader
