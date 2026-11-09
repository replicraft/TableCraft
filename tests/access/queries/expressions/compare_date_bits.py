"""Verify ordinary/cached DATE serial bits against independent precision probes."""
import argparse
import hashlib
from pathlib import Path
import struct

from compare_composite_constraints import run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "rows", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    bits = [0, 0x8000000000000000, 1, 0x8000000000000001]
    bits += [struct.unpack("<Q", struct.pack("<d", value))[0] for value in (
        0.25, -0.25, -1.25, -1.75, 1.0000000001, -1.0000000001,
        45000.123456789, -657434.25, 2958465.9999999995)]
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        for table in ("ExactDates", "CalculatedExactDates"):
            expected = "".join(f"{table}\tY64:{value:016x}\n" for value in bits) + table + "\tN\n"
            oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/StoredDateBitsOracle.java", path, table, "Value")
            if oracle != expected:
                raise RuntimeError("Independent DATE oracle rounded precision/alias probes: " + table)
            if table == "CalculatedExactDates":
                actual = run(args.rows, path, table, "--stored-calculation", "Value", "--date-bits")
            else:
                rows = run(args.rows, path, table, "--date-bits")
                actual = "".join(table + "\t" + row.split("\t")[2] + "\n" for row in rows.splitlines())
            if actual != expected:
                raise RuntimeError("Native exact DATE bits differ: " + table)
            normalized += actual
        if path.read_bytes() != original:
            raise RuntimeError("Original DATE precision input changed")
        print(path.name, "exact cells", 28, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact ordinary/cached DATE serialization, signed zero, aliases and sub-millisecond probes passed; native Access unrun.")


if __name__ == "__main__":
    main()
