#!/usr/bin/env python3
"""Compare explicit ICU4X text profiles against independent ICU4C calls, not NLS."""

import argparse
import csv
import ctypes
import ctypes.util
import hashlib
import io
import itertools
import json
import random
import subprocess
import sys
from pathlib import Path


LOCALES = {"en-US": 1033, "de-DE": 1031, "sv-SE": 1053, "tr-TR": 1055}
UCOL_ALTERNATE_HANDLING = 1
UCOL_CASE_LEVEL = 3
UCOL_NORMALIZATION_MODE = 4
UCOL_STRENGTH = 5
UCOL_NUMERIC_COLLATION = 7
UCOL_SECONDARY = 1
UCOL_OFF = 16
UCOL_ON = 17
UCOL_NON_IGNORABLE = 21
BATCH_COLUMNS = 48


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def units(text):
    return len(text.encode("utf-16-le")) // 2


def quoted(text):
    return "&ChrW(0)&".join("'" + part.replace("'", "''") + "'" for part in text.split("\0"))


class Reference:
    def __init__(self, library):
        self.library = ctypes.CDLL(library)
        if sys.byteorder != "little":
            raise RuntimeError("reference UTF-16 bridge currently requires a little-endian host")
        self.open = self.symbol("ucol_open", [ctypes.c_char_p, ctypes.POINTER(ctypes.c_int32)], ctypes.c_void_p)
        self.close = self.symbol("ucol_close", [ctypes.c_void_p], None)
        self.attribute = self.symbol("ucol_setAttribute", [ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32, ctypes.POINTER(ctypes.c_int32)], None)
        pointer = ctypes.POINTER(ctypes.c_uint16)
        self.compare = self.symbol("ucol_strcoll", [ctypes.c_void_p, pointer, ctypes.c_int32, pointer, ctypes.c_int32], ctypes.c_int32)
        self.version = self.symbol("u_getVersion", [ctypes.POINTER(ctypes.c_uint8)], None)
        self.collation_version = self.symbol("ucol_getVersion", [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint8)], None)

    def symbol(self, name, arguments, result):
        for suffix in [""] + [f"_{version}" for version in range(100, 49, -1)]:
            function = getattr(self.library, name + suffix, None)
            if function is not None:
                function.argtypes = arguments
                function.restype = result
                return function
        raise RuntimeError(f"missing ICU4C API {name}")

    def collator(self, locale):
        status = ctypes.c_int32(0)
        collator = self.open(locale.encode("ascii"), ctypes.byref(status))
        if not collator or status.value > 0:
            raise RuntimeError("ICU4C locale initialization failed")
        for attribute, value in [
            (UCOL_ALTERNATE_HANDLING, UCOL_NON_IGNORABLE),
            (UCOL_CASE_LEVEL, UCOL_OFF),
            (UCOL_NORMALIZATION_MODE, UCOL_ON),
            (UCOL_STRENGTH, UCOL_SECONDARY),
            (UCOL_NUMERIC_COLLATION, UCOL_OFF),
        ]:
            status.value = 0
            self.attribute(collator, attribute, value, ctypes.byref(status))
            if status.value > 0:
                self.close(collator)
                raise RuntimeError("ICU4C attribute selection failed")
        return collator

    def order(self, collator, left, right):
        left_bytes = left.encode("utf-16-le")
        right_bytes = right.encode("utf-16-le")
        left_units = (ctypes.c_uint16 * (len(left_bytes) // 2)).from_buffer_copy(left_bytes)
        right_units = (ctypes.c_uint16 * (len(right_bytes) // 2)).from_buffer_copy(right_bytes)
        result = self.compare(collator, left_units, len(left_units), right_units, len(right_units))
        return (result > 0) - (result < 0)

    def search(self, collator, haystack, needle, start, reverse):
        length = units(haystack)
        if start == -1 and reverse:
            start = length
        if length == 0 or start > length:
            return 0
        if not needle:
            return start
        positions = list(range(len(haystack)))
        if reverse:
            positions.reverse()
        for position in positions:
            unit_start = units(haystack[:position])
            if (not reverse and unit_start < start - 1) or (reverse and unit_start >= start):
                continue
            for end in range(position + 1, len(haystack) + 1):
                if reverse and units(haystack[:end]) > start:
                    break
                if self.order(collator, haystack[position:end], needle) == 0:
                    return unit_start + 1
        return 0


def cases():
    anchors = ["", "a", "A", "à", "é", "é", "Ⓐ", "ﬁ", "fi", "I", "i", "İ", "ı", "z", "ö", "ä", "å", "😀", "\0", "a-b", "ab", "a ", "a'b"]
    pairs = list(itertools.product(anchors, repeat=2))
    rng = random.Random(0x49435554455854)
    alphabet = "aAbBeéöIıİﬁ😀́"
    pairs.extend(("".join(rng.choices(alphabet, k=6)), "".join(rng.choices(alphabet, k=6))) for _ in range(64))
    searches = [("😀AéZ", "É"), ("xⒶy", "a"), ("xxﬁz", "fi"), ("xFIz", "ﬁ"), ("aAa", "A"), ("Iİıi", "i"), ("x\0a", "\0"), ("", "a"), ("abc", "")]
    searches.extend(("".join(rng.choices(alphabet, k=8)), rng.choice(alphabet)) for _ in range(24))
    return pairs, searches


def verify(cli, locale, expressions, expected, extra=()):
    count = 0
    for at in range(0, len(expressions), BATCH_COLUMNS):
        sql = "SELECT " + ",".join(expressions[at:at + BATCH_COLUMNS])
        result = subprocess.run([str(cli), "query", "run", sql, "--text-locale", locale, "--comparison-steps", "100000000", *extra], capture_output=True, text=True)
        if result.returncode != 0:
            raise RuntimeError(f"ICU CLI profile failed: {result.stderr.strip()}")
        rows = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
        actual = [int(value) for value in rows[1]] if len(rows) == 2 else []
        wanted = expected[at:at + BATCH_COLUMNS]
        if actual != wanted:
            mismatch = next((i for i, pair in enumerate(zip(actual, wanted)) if pair[0] != pair[1]), 0)
            raise RuntimeError(f"ICU version/profile disagreement at {locale} control {at + mismatch}; do not treat this as native qualification")
        count += len(wanted)
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--icu-library", default=ctypes.util.find_library("icucore") or ctypes.util.find_library("icui18n"))
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    if not args.icu_library:
        parser.error("an explicit available ICU4C library is required")
    if args.report and args.report.exists():
        parser.error("report must be fresh")
    binary = digest(args.cli)
    reference = Reference(args.icu_library)
    version = (ctypes.c_uint8 * 4)()
    reference.version(version)
    pairs, searches = cases()
    controls = 0
    versions = {}
    for locale, lcid in LOCALES.items():
        collator = reference.collator(locale)
        try:
            coll_version = (ctypes.c_uint8 * 4)()
            reference.collation_version(collator, coll_version)
            versions[locale] = list(coll_version)
            comparisons = [f"StrComp({quoted(left)},{quoted(right)},1)" for left, right in pairs]
            expected = [reference.order(collator, left, right) for left, right in pairs]
            controls += verify(args.cli, locale, comparisons, expected)
            expressions, expected = [], []
            for haystack, needle in searches:
                for start, reverse in [(1, False), (2, False), (-1, True), (1, True)]:
                    call = f"InStrRev({quoted(haystack)},{quoted(needle)},{start},1)" if reverse else f"InStr({start},{quoted(haystack)},{quoted(needle)},1)"
                    expressions.append(call)
                    expected.append(reference.search(collator, haystack, needle, start, reverse))
            controls += verify(args.cli, locale, expressions, expected)
            expressions = [f"InStr(1,{quoted(haystack)},{quoted(needle)},{lcid})" for haystack, needle in searches]
            expected = [reference.search(collator, haystack, needle, 1, False) for haystack, needle in searches]
            controls += verify(args.cli, locale, expressions, expected, ("--text-lcid", str(lcid)))
        finally:
            reference.close(collator)
    if digest(args.cli) != binary:
        raise RuntimeError("CLI changed during independent comparison")
    report = {
        "controls": controls, "locales": list(LOCALES), "icu4c_version": list(version),
        "icu4c_collation_versions": versions, "binary_sha256": binary,
        "runner_sha256": digest(__file__), "inputs_sha256": hashlib.sha256(json.dumps([pairs, searches], ensure_ascii=True).encode()).hexdigest(),
        "profile": "secondary, case level off, normalization on, nonignorable punctuation, numeric off",
        "reference": "ICU4C ucol_strcoll; independent scalar-boundary span enumeration",
        "native_access": "unrun", "windows_nls": "unrun",
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        with args.report.open("x") as output:
            json.dump(report, output, indent=2, sort_keys=True)
            output.write("\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
