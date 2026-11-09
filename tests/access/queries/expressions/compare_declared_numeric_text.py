"""Independent rounding/width models for declared numeric/String predicates.

Only TableCraft executes SQL; native getters supply stored integers, not answers.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
import json
from pathlib import Path
import struct
import subprocess

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
INPUTS = {
    "Scalar-V2000.mdb": "de2e67a9f0194988d031cc9c981176ae9a76d3bf80599040729b408fc2b7be82",
    "Scalar-V2007.accdb": "4ed9555c239e56f0649040fb9357913e4e38dcbaac5c2cb668614857882922bc",
    "Scalar-V2016.accdb": "95c52665d28869a92764f0958b62ad53e465847e3a0c637829d4ff29c8f5ef37",
    "Scalar-V2019.accdb": "eef6a7d1f23c482333ad5b70b312125bd89799ea168b10a522491eb4b7625190",
}
BOUNDS = {
    "CByte": (0, 255),
    "CInt": (-32768, 32767),
    "CLng": (-2147483648, 2147483647),
    "CLngLng": (-9223372036854775808, 9223372036854775807),
}
OPERATORS = ("=", "<>", "<", "<=", ">", ">=")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def convert(kind, text):
    value = Decimal(text)
    if kind in BOUNDS:
        value = int(value.to_integral_value(rounding=ROUND_HALF_EVEN))
        if not BOUNDS[kind][0] <= value <= BOUNDS[kind][1]:
            raise OverflowError
        return value
    if kind == "CBool":
        return -1 if value else 0
    value = float(value)
    return struct.unpack("!f", struct.pack("!f", value))[0] if kind == "CSng" else value


def truths(left, right):
    if left is None or right is None:
        return ["NULL"] * len(OPERATORS)
    results = (left == right, left != right, left < right, left <= right, left > right, left >= right)
    return ["TRUE" if result else "FALSE" for result in results]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Report must be fresh"
    assert digest(args.jar) == JAR_SHA256
    oracle = Path("docs/testing/TableDataOracle.java")
    pins = (args.cli, args.jar, Path(__file__).resolve(), oracle)
    record = {
        "scope": __doc__,
        "pins": [{"path": str(path), "sha256": digest(path)} for path in pins],
        "inputs": [],
        "controls": [],
    }

    def query(label, expressions, expected, profile, database=None, overflow=False):
        sql = "SELECT " + ",".join(f"{expression} AS V{index}" for index, expression in enumerate(expressions))
        if database is not None:
            sql += " FROM ScalarSamples ORDER BY ID"
        command = [
            str(args.cli), "query", "run", sql, "--declared-arithmetic",
            "--number-format", profile, "--format", "tsv",
        ]
        if database is not None:
            command.extend(("--database", str(database)))
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=60)
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        header = [f"V{index}" for index in range(len(expressions))]
        passed = (result.returncode == 2 and not result.stdout and "overflow" in result.stderr) if overflow else (
            result.returncode == 0 and not result.stderr and actual == [header, *expected]
        )
        record["controls"].append({
            "label": label,
            "command": command,
            "passed": passed,
            "exit_code": result.returncode,
            "stderr": result.stderr,
            "expected_cells": sum(map(len, expected)),
            "expected_sha256": hashlib.sha256(json.dumps(expected).encode()).hexdigest(),
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
        })
        args.report.write_text(json.dumps(record, indent=2) + "\n")
        assert passed, (label, result.stderr, actual[:3], expected[:3])

    for profile in ("en-us", "de-de"):
        for kind, numeric, texts in (
            ("CByte", "2", ("1.5", "2.5", "3.5", "255.5")),
            ("CInt", "-2", ("-1.5", "-2.5", "3.5", "32768")),
            ("CLng", "2", ("1.5", "2.5", "3.5", "2147483648")),
            ("CLngLng", "9007199254740993", ("9007199254740992", "9007199254740993", "9007199254740994", "9223372036854775808")),
            ("CBool", "1", ("0", "0.5", "1.5", "-1")),
            ("CSng", "16777216", ("16777215", "16777216", "16777217", "16777218")),
            ("CDbl", "0.5", ("0.25", "0.5", "0.75", "1")),
        ):
            local_numeric = numeric.replace(".", ",") if profile == "de-de" else numeric
            left = f"{kind}('{local_numeric}')"
            for text in texts:
                local = text.replace(".", ",") if profile == "de-de" else text
                expressions = []
                expected = []
                try:
                    target, converted = convert(kind, numeric), convert(kind, text)
                    overflow = False
                except OverflowError:
                    target, converted, overflow = 0, 0, True
                for peer in (f"'{local}'", f"CVar('{local}')", f"First('{local}')"):
                    for a, b, x, y in ((left, peer, target, converted), (peer, left, converted, target)):
                        expressions.extend(f"{a}{operator}{b}" for operator in OPERATORS)
                        expected.extend(truths(x, y))
                query(f"{profile}:{kind}:{text}", expressions, [] if overflow else [expected], profile, overflow=overflow)
        print(profile, "source-free controls passed", flush=True)

    for name, expected_hash in INPUTS.items():
        path = args.directory / name
        assert digest(path) == expected_hash, "Unregistered native input"
        raw = subprocess.check_output(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), str(oracle), str(path), "ScalarSamples"],
            text=True,
            encoding="utf-8",
            timeout=60,
        )
        rows = [line.split("\t") for line in raw.splitlines()]
        assert len(rows) == 3
        record["inputs"].append({
            "path": str(path),
            "sha256": expected_hash,
            "getter_sha256": hashlib.sha256(raw.encode()).hexdigest(),
        })
        for column, kind, position, texts in (
            ("ID", "CLng", 1, ("1.5", "2.5", "2147483648")),
            ("Octet", "CByte", 3, ("0.5", "254.5", "255.5")),
            ("Small", "CInt", 4, ("-32768.4", "32767.4", "32768")),
        ):
            for profile in ("en-us", "de-de"):
                for text in texts:
                    local = text.replace(".", ",") if profile == "de-de" else text
                    expressions = [f"{column}{operator}'{local}'" for operator in OPERATORS]
                    try:
                        converted, overflow = convert(kind, text), False
                    except OverflowError:
                        converted, overflow = 0, True
                    expected = []
                    for row in rows:
                        tagged = row[position]
                        numeric = None if tagged == "N" else int(tagged.removeprefix("U").split(":")[-1])
                        expected.append(truths(numeric, converted))
                    query(f"{name}:{profile}:{column}:{text}", expressions, [] if overflow else expected,
                          profile, path, overflow)
        assert digest(path) == expected_hash
        record["inputs"][-1]["unchanged"] = True
        print(name, "native controls passed", flush=True)

    for pin in record["pins"]:
        assert digest(pin["path"]) == pin["sha256"]
        pin["path"] = str(Path(pin["path"]).resolve().relative_to(Path.cwd()))
    record["all_pins_unchanged"] = True
    record["control_count"] = len(record["controls"])
    args.report.write_text(json.dumps(record, indent=2) + "\n")
    print(record["control_count"], "controls passed; native Access unrun")


if __name__ == "__main__":
    main()
