"""Independent IEEE result/carrier checks for explicit declared arithmetic.

Only TableCraft executes SQL; raw native getters do not qualify Access evaluation.
"""
import argparse
import csv
import hashlib
from io import StringIO
import itertools
import math
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256


def model(operator, a, b, single_selected, variant):
    if operator == "/" and b == 0:
        return None
    value = {"+": lambda: a + b, "-": lambda: a - b,
             "*": lambda: a * b, "/": lambda: a / b}[operator]()
    if not math.isfinite(value):
        return None
    if single_selected:
        try:
            bits = struct.pack("!f", value)
        except OverflowError:
            return (5, struct.pack("!d", value)) if variant else None
        if math.isfinite(struct.unpack("!f", bits)[0]):
            return 4, bits
        return (5, struct.pack("!d", value)) if variant else None
    return 5, struct.pack("!d", value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def query(expression, expected, options=(), source=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N,VarType({expression}) AS T"
        if source is not None:
            sql += " FROM IndexedRows WHERE ID=2"
            options = [*options, "--database", source, "--index", "IndexedRows", "IdAsc",
                       "--native-bytes", "67108864", "--native-rows", "100000",
                       "--native-steps", "10000000", "--execution-steps", "10000000"]
        result = subprocess.run([str(args.cli), "query", "run", sql,
                                 "--declared-arithmetic", *map(str, options)],
                                capture_output=True, text=True, encoding="utf-8", check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert len(rows) == 2 and rows[0] == ["N", "T"], (sql, rows)
            tag, bits = expected
            assert rows[1][1] == str(tag), (sql, rows, expected)
            actual = struct.pack("!f" if tag == 4 else "!d", float(rows[1][0]))
            assert actual == bits, (sql, rows, actual.hex(), bits.hex())
        controls += 1

    values = [struct.unpack("!f", bits.to_bytes(4, "big"))[0] for bits in [
        0, 0x80000000, 1, 0x80000001, 0x00800000, 0x3EAAAAAB,
        0x3F000000, 0x3F800000, 0x7F7FFFFF, 0xFF7FFFFF]]
    maximum = struct.unpack("!f", bytes.fromhex("7f7fffff"))[0]
    half_overflow_ulp = float(2**103)
    right = [("CByte(2)", 2.0, True), ("CInt(-1)", -1.0, True),
             ("CSng(0.5)", 0.5, True), ("CSng(0)", 0.0, True),
             ("CLng(2)", 2.0, False), ("CDbl(2)", 2.0, False),
             (f"CSng({maximum!r})", maximum, True),
             (f"CSng({half_overflow_ulp!r})", half_overflow_ulp, True)]
    for a, (r, b, selected), operator, av, bv in itertools.product(
            values, right, ["+", "-", "*", "/"], [False, True], [False, True]):
        left = f"CSng({a!r})"
        left = f"CVar({left})" if av else left
        r = f"CVar({r})" if bv else r
        query(f"{left}{operator}{r}", model(operator, a, b, selected, av or bv))

    maximum = struct.unpack("!f", bytes.fromhex("7f7fffff"))[0]
    minimum = struct.unpack("!f", bytes.fromhex("00000001"))[0]
    for suffix, digest in INPUT_HASHES.items():
        source = args.directory / ("index-" + suffix)
        original = source.read_bytes()
        assert hashlib.sha256(original).hexdigest() == digest
        output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(source), "IndexedRows"], text=True, encoding="utf-8")
        selected = [line.split("\t")[7] for line in output.splitlines()
                    if line.split("\t")[1] == "I32:2"]
        assert len(selected) == 1 and selected[0].startswith("F")
        a = struct.unpack("!f", bytes.fromhex(selected[0][1:]))[0]
        for operator, b, text in [("*", maximum, f"CSng({maximum!r})"),
                                  ("/", minimum, f"CSng({minimum!r})")]:
            query(f"SingleValue{operator}{text}", model(operator, a, b, True, False), source=source)
            query(f"CVar(SingleValue){operator}{text}", model(operator, a, b, True, True), source=source)
        query("SingleValue*CLng(2)", model("*", a, 2.0, False, False), source=source)
        assert source.read_bytes() == original
    print(f"Passed {controls} independent declared-floating controls; native Access remains unqualified")


if __name__ == "__main__":
    main()
