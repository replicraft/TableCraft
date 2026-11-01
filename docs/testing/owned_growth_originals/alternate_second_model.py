"""Literal same-size second request on an independently authored cross-page row."""
import base64
import hashlib


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def construct(before, original_getter, profile):
    require(len(before) == profile["bytes"] and sha256(before) == profile["sha256"],
            "Frozen source-derived alternate second BEFORE")
    root, target = profile["root"] * 4096, profile["target"] * 4096
    require(before[root + 2:root + 4] == bytes([178, 13])
            and before[root + 12:root + 16] == bytes([255, 0, 190, 79]),
            "Original saturated forwarded root page")
    require(before[root + 4030:root + 4096] == (profile["target"] << 8).to_bytes(4, "little") + bytes(62),
            "Original cross-page forwarding capsule")
    require(before[target + 2:target + 4] == bytes([172, 15])
            and before[target + 12:target + 16] == bytes([1, 0, 188, 143]),
            "Original owned target page has one68B hidden terminal")
    require(before[target + 4028:target + 4034] == bytes([3, 0, 23, 0, 0, 0])
            and before[target + 4034:target + 4072] == b"a\x00" * 19,
            "Exact original ID and Marker payload")
    require(before[target + 4072:target + 4096] == bytes.fromhex(
        "030000800000000000000000c700ff3b002c000600020007"), "Exact unchanged OLE/footer/mask")
    start, end = target + 4034, target + 4072
    after = before[:start] + b"b\x00" * 19 + before[end:]
    require(after[:start] == before[:start] and after[end:] == before[end:], "Complete complement")
    lines = []
    edits = 0
    for line in original_getter.splitlines(keepends=True):
        fields = line[:-1].split("\t")
        if fields[:3] == ["ROW", "Sentinel", "0"]:
            require(len(fields) == 7 and fields[3] == f"{profile['root']}:0" and fields[4] == "I32:23"
                    and fields[5] == "T:" + base64.b64encode(b"unchanged sentinel").decode(),
                    "Original independent logical-row observation")
            fields[5] = "T:" + base64.b64encode(b"b" * 19).decode()
            line = "\t".join(fields) + "\n"
            edits += 1
        lines.append(line)
    require(edits == 1, "One logical Marker change")
    reader = "".join(lines)
    proof = {"before_sha256": sha256(before), "expected_sha256": sha256(after),
             "reader_sha256": sha256(reader.encode()), "changed_rows": 1,
             "range": [start, end], "expected": "a" * 19, "replacement": "b" * 19,
             "all_other_bytes_root_target_frames_maps_eof_unchanged": True,
             "native_producer_calls": 0}
    return after, proof, reader
