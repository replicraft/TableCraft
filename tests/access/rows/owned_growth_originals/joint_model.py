"""Original-only joint fixed/variable literal relocated-record images."""
import base64
import hashlib

OLD_RECORD = bytes.fromhex(
    "03001700000075006e006300680061006e006700650064002000730065006e00740069006e0065006c00"
    "030000800000000000000000c700ff39002a000600020007"
)
INLINE = bytes.fromhex("030000800000000000000000c700ff")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def construct(raw, getter, profile, recipe):
    require(len(raw) == profile["bytes"] and sha256(raw) == profile["sha256"], "Frozen actual original")
    page = profile["data"]
    base = page * 4096
    original = raw[base:base + 4096]
    require(original[:2] == b"\x01\x01" and int.from_bytes(original[4:8], "little") == profile["definition"],
            "Original native DATA owner")
    require(original[2:4] == bytes([174, 15]) and original[12:16] == bytes([1, 0, 190, 15])
            and original[4030:] == OLD_RECORD, "Original free4014/direct row66B")
    properties = [line.split("\t") for line in getter.splitlines()
                  if line.startswith("PROPERTY\tSentinel\t")]
    require(properties == [["PROPERTY", "Sentinel", "", "PublishToWeb", "2", "0", "U8:0"]],
            "Complete independent Sentinel properties have no Required ID restriction")
    require(recipe["id"] in ["id-null", "id-twenty-four"], "Closed literal requests")
    null = recipe["id"] == "id-null"
    # NULL retains actual original Int32 bytes; only the new mask's bit0 clears.
    fixed = OLD_RECORD[:6] if null else bytes([3, 0, 24, 0, 0, 0])
    terminal = fixed + b"a\x00" * 19 + INLINE + bytes([59, 0, 44, 0, 6, 0, 2, 0, 6 if null else 7])
    require(len(terminal) == 68 and terminal[44:59] == OLD_RECORD[42:57], "Literal resized record/OLE spans")
    require(terminal[67] == recipe["mask"], "Presence mask uses new byte67, not original65")
    after_page = bytearray(original)
    after_page[3962:4030] = terminal
    after_page[4030:4096] = (page << 8 | 1).to_bytes(4, "little") + bytes(62)
    after_page[2:4] = bytes([104, 15])
    after_page[12:18] = bytes([2, 0, 190, 79, 122, 143])
    require(after_page[18:3962] == original[18:3962]
            and after_page[4:12] == original[4:12], "Complete untouched DATA extents")
    after = raw[:base] + bytes(after_page) + raw[base + 4096:]
    lines = []
    edits = 0
    for line in getter.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(len(cells) == 7 and cells[3] == f"{page}:0" and cells[4] == "I32:23"
                    and cells[5] == "T:" + base64.b64encode(b"unchanged sentinel").decode(),
                    "Exact independent original row request")
            cells[4] = "N" if null else "I32:24"
            cells[5] = "T:" + base64.b64encode(b"a" * 19).decode()
            line = "\t".join(cells) + "\n"
            edits += 1
        lines.append(line)
    require(edits == 1, "One logical root with two changed cells")
    reader = "".join(lines)
    return after, {"source_sha256": sha256(raw), "expected_sha256": sha256(after),
                   "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
                   "replacement_id": recipe["replacement"], "id_before": 23,
                   "marker_expected": "unchanged sentinel", "marker_replacement": "a" * 19,
                   "mask_record_offset": 67, "mask": terminal[67], "fixed_id_hex": terminal[2:6].hex(),
                   "hidden_span": [base + 3962, base + 4030], "root_span": [base + 4030, base + 4096],
                   "all_other_pages_and_ole_properties_maps_unchanged": True,
                   "native_producer_calls": 0}, reader
