"""Independent bounded Latin weight-stream model; no TableCraft imports."""

import argparse
import hashlib
import json
from pathlib import Path

NEUTRAL_SECONDARY = 2
PRIMARY_END = 1
KEY_END = 0
LONG_LENGTHS = (256, 257, 1024, 20000)


def decode_hex(value):
    return b"" if value == "-" else bytes.fromhex(value)


def load(path):
    weights = {}
    short = []
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "weight":
            _, code, kind, units, primary_units, placeholder, modifier, crazy, inline, extra, unprintable, tertiary = fields
            assert kind in ("SIMPLE", "INTERNATIONAL"), fields
            assert (int(units), int(primary_units), int(placeholder), int(modifier), int(crazy)) == (1, 1, 2, 0, 0), fields
            assert unprintable == tertiary == "-", fields
            assert len(decode_hex(extra)) <= 1, fields
            weights[chr(int(code, 16))] = (decode_hex(inline), decode_hex(extra))
        else:
            assert fields[0] == "short", fields
            encoded = decode_hex(fields[1])
            value = encoded.decode("utf-16-be")
            short.append((value, decode_hex(fields[2])))
    return weights, short


def model_key(value, weights):
    # The only padding equivalence in this engineering comparison contract.
    value = value.rstrip(" ")
    primary = bytearray()
    secondary = bytearray()
    for unit in value:
        inline, accent = weights[unit]
        primary.extend(inline)
        secondary.extend(accent or bytes([NEUTRAL_SECONDARY]))
    secondary.reverse()
    while secondary and secondary[-1] <= NEUTRAL_SECONDARY:
        secondary.pop()
    return bytes(primary + bytes([PRIMARY_END]) + secondary + bytes([KEY_END]))


def pairs():
    for length in LONG_LENGTHS:
        yield f"suffix-primary-{length}", "a" * length + "b", "a" * length + "c"
        yield f"backwards-secondary-{length}", "é" + "a" * length + "è", "è" + "a" * length + "é"
        yield f"neutral-padding-{length}", "é" + "a" * length + "e", "e" + "a" * length + "é"
        yield f"trailing-space-{length}", "é" + "a" * length, "é" + "a" * length + "  "
        yield f"case-{length}", "é" + "a" * length, "É" + "A" * length


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--vectors", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    weights, short = load(args.oracle)
    for number, (value, expected) in enumerate(short):
        actual = model_key(value, weights)
        assert actual == expected, (number, value, expected.hex(), actual.hex())
    output = []
    outcomes = []
    for label, left, right in pairs():
        left_key = model_key(left, weights)
        right_key = model_key(right, weights)
        ordering = (left_key > right_key) - (left_key < right_key)
        output.append("\t".join((label, left.encode("utf-16-be").hex(), right.encode("utf-16-be").hex(), str(ordering))))
        outcomes.append({"label": label, "left_units": len(left), "right_units": len(right), "ordering": ordering})
    args.vectors.write_text("\n".join(output) + "\n")
    report = {
        "scope": "Independent long-Latin engineering weight-stream model; not a long Jackcess string comparator or native Access/NLS evidence",
        "descriptor": [1036, 0, 0],
        "measured_latin_handlers": len(weights),
        "short_complete_key_cross_checks": len(short),
        "long_pairs": outcomes,
        "unprintable_full_positions": "outside this independent oracle",
        "oracle_sha256": hashlib.sha256(args.oracle.read_bytes()).hexdigest(),
        "vectors_sha256": hashlib.sha256(args.vectors.read_bytes()).hexdigest(),
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"handlers": len(weights), "short_keys": len(short), "long_pairs": len(outcomes)}))


if __name__ == "__main__":
    main()
