"""Literal same-length second edit on independently authored forwarded BEFORE."""
import base64
import hashlib


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def construct(before, original_getter, profile):
    require(len(before) == profile["bytes"] and sha256(before) == profile["sha256"],
            "Frozen source-derived second BEFORE")
    base = profile["data"] * 4096
    require(before[base + 2:base + 4] == bytes([104, 15])
            and before[base + 12:base + 18] == bytes([2, 0, 190, 79, 122, 143]),
            "Actual forwarded geometry: free3944, two slots")
    pointer = (profile["data"] << 8 | 1).to_bytes(4, "little")
    require(before[base + 4030:base + 4096] == pointer + bytes(62), "Root capsule")
    require(before[base + 3962:base + 3968] == bytes([3, 0, 23, 0, 0, 0]), "Literal terminal ID")
    require(before[base + 3968:base + 4006] == b"a\x00" * 19, "Original nineteen-unit Marker")
    require(before[base + 4006:base + 4030] == bytes.fromhex(
        "030000800000000000000000c700ff3b002c000600020007"), "Readonly OLE and footer")
    start, end = base + 3968, base + 4006
    after = before[:start] + b"b\x00" * 19 + before[end:]
    require(after[:start] == before[:start] and after[end:] == before[end:], "Complete complement")
    output = []
    edits = 0
    for line in original_getter.splitlines(keepends=True):
        cells = line[:-1].split("\t")
        if cells[:3] == ["ROW", "Sentinel", "0"]:
            require(cells[3] == f"{profile['data']}:0" and len(cells) == 7
                    and cells[5] == "T:" + base64.b64encode(b"unchanged sentinel").decode(),
                    "Original independent logical row authority")
            cells[5] = "T:" + base64.b64encode(b"b" * 19).decode()
            line = "\t".join(cells) + "\n"
            edits += 1
        output.append(line)
    require(edits == 1, "Exactly one logical Marker observation")
    reader = "".join(output)
    return after, {"before_sha256": sha256(before), "expected_sha256": sha256(after),
                   "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
                   "range": [start, end], "expected": "a" * 19, "replacement": "b" * 19,
                   "same_root_hidden_footer_maps_and_all_other_bytes": True,
                   "native_producer_calls": 0}, reader
