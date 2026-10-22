"""Check exact UNION widening with independent integers/IEEE and native stored rows.

This is a TableCraft profile/value model, not native Access result-type evidence.
"""
import argparse
import csv
import hashlib
from io import StringIO
import math
import json
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES

INTEGER_TYPES = [("CByte", 0, 255, "Byte"), ("CInt", -32768, 32767, "Integer"),
                 ("CLng", -2147483648, 2147483647, "Long"),
                 ("CLngLng", -9223372036854775808, 9223372036854775807, "LongLong")]
NATIVE_HASHES = {
    "jet4.mdb": "0294a3fe8ddb560776f71e7f9b058bd86539aead04514b13fcf9d89ddb09b3e9",
    "2007.accdb": "199ff8a12682a50f4d1fc696d7db6f5e1b5313fd65f03abc7126c08bd9336aee",
    "2016.accdb": "c131ed71b372ebb028d6816381c3984a68c89394eeb7df55baecb9dc1465858c",
    "2019.accdb": "b5509cfd4d9e7bd286531756db148eafdf29d51e79f606902b05288fc4049e8e",
}
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--jar", type=Path)
    args = parser.parse_args()
    if bool(args.native_directory) != bool(args.jar):
        parser.error("--native-directory and --jar must be supplied together")
    controls = 0

    def query(sql, options=()):
        result = subprocess.run([str(args.cli), "query", "run", sql, *map(str, options)], text=True, encoding="utf-8", capture_output=True)
        assert result.returncode == 0, (sql, result.stderr)
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        assert rows[0] == ["N", "Carrier"], rows
        data = rows[1:]
        for row in data:
            row[1] = json.loads(row[1])  # The CLI retains explicit Text literal quoting inside TSV.
        return data

    def wrapped(body):
        return f"SELECT U.N AS N,TypeName(U.N) AS Carrier FROM ({body}) AS U ORDER BY U.N"

    for left_index, (left, minimum, _, _) in enumerate(INTEGER_TYPES):
        for right_index, (right, _, maximum, _) in enumerate(INTEGER_TYPES):
            carrier = INTEGER_TYPES[max(left_index, right_index)][3]
            for a, b in ((minimum, maximum), (1, 1)):
                for all_rows in (False, True):
                    body = f"SELECT {left}({a}) AS N UNION {'ALL' if all_rows else ''} SELECT {right}({b})"
                    values = sorted([a, b] if all_rows else set([a, b]))
                    assert query(wrapped(body)) == [[str(value), carrier] for value in values], body
                    controls += 1
    for text in ("-0.0", "1e-45", "0.1", "16777215", "3.4028234e38"):
        single = struct.unpack("f", struct.pack("f", float(text)))[0]
        body = "SELECT -CSng(0) AS N UNION ALL SELECT -CDbl(0)" if text == "-0.0" else f"SELECT CSng({text}) AS N UNION ALL SELECT CDbl({text})"
        rows = query(wrapped(body))
        expected = sorted([single, float(text)])
        assert len(rows) == 2 and all(row[1] == "Double" for row in rows)
        actual = [float(row[0]) for row in rows]
        assert actual == expected, (text, actual, expected)
        assert [math.copysign(1, value) for value in actual] == [math.copysign(1, value) for value in expected]
        controls += 1

    # Stable arithmetic descriptors must participate in the existing UNION
    # carrier selection, including when no literal/conversion is the arm root.
    for expression, other, expected, carrier in [
        ("CLngLng(9007199254740993)+CLngLng(2)", "CByte(1)", [1, 9007199254740995], "LongLong"),
        ("CDbl(1)+CInt(2)", "CSng(4)", [3, 4], "Double"),
        ("CByte(2)/CByte(1)", "CSng(4)", [2, 4], "Double"),
        ("CInt(2)^CInt(3)", "CSng(4)", [4, 8], "Double"),
        ("CInt(3) Mod CInt(2)", "CByte(2)", [1, 2], "Integer"),
        ("CLng(5) Mod CLng(2)", "CInt(2)", [1, 2], "Long"),
        ("CByte(4)\\CByte(2)", "CInt(4)", [2, 4], "Integer"),
        ("CDate(3)-CDate(1)", "CSng(4)", [2, 4], "Double"),
    ]:
        for left, right in [(expression, other), (other, expression)]:
            body = f"SELECT {left} AS N UNION ALL SELECT {right}"
            assert query(wrapped(body)) == [[str(value), carrier] for value in expected], body
            controls += 1

    if args.native_directory:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in NATIVE_HASHES.items():
            path = args.native_directory / ("unicode-" + suffix)
            original = path.read_bytes()
            assert hashlib.sha256(original).hexdigest() == digest
            stored = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "AData", "BData"], text=True, encoding="utf-8")
            inputs = {name: [] for name in ("AData", "BData")}
            for line in stored.splitlines():
                name, value = line.split("\t")
                inputs[name].append(int(value.removeprefix("I32:")))
            a, b = inputs["AData"], inputs["BData"]
            for body, expected, carrier in [
                ("SELECT ID AS N FROM AData UNION ALL SELECT CByte(255)", a + [255], "Long"),
                ("SELECT CInt(ID) AS N FROM AData UNION ALL SELECT ID FROM BData", a + b, "Long"),
                ("SELECT ID AS N FROM AData UNION SELECT CInt(ID) FROM BData", set(a + b), "Long"),
                ("SELECT ID AS N FROM AData UNION ALL SELECT CLngLng(9007199254740993)", a + [9007199254740993], "LongLong"),
            ]:
                assert query(wrapped(body), ["--database", path]) == [[str(value), carrier] for value in sorted(expected)], (path, body)
                controls += 1
            for body, expected, carrier in [
                ("SELECT CLngLng(ID)+1 AS N FROM AData UNION ALL SELECT CByte(255)",
                 [value + 1 for value in a] + [255], "LongLong"),
                ("SELECT ID/2 AS N FROM AData UNION ALL SELECT CSng(4)",
                 [value / 2 for value in a] + [4], "Double"),
            ]:
                actual = query(wrapped(body), ["--database", path])
                assert [float(value) for value, _ in actual] == sorted(expected), (path, body)
                assert all(kind == carrier for _, kind in actual), (path, body)
                controls += 1
            assert path.read_bytes() == original
    print(f"Passed {controls} exact UNION widening/value controls; native Access type qualification unrun.")


if __name__ == "__main__":
    main()
