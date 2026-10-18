"""Compare bounded native reads with independently filtered Jackcess cursor rows."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--example", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--ace", required=True, type=Path)
    parser.add_argument("--jet", required=True, type=Path)
    args = parser.parse_args()
    cases = [
        ("zero", ["range", "--lower", "0", "--upper", "0"], lambda v: v == 0),
        ("closed", ["range", "--lower", "-3", "--upper", "4"], lambda v: -3 <= v <= 4),
        ("open", ["range", "--lower", "-1", "--exclude-lower", "--upper", "3", "--exclude-upper"],
         lambda v: -1 < v < 3),
        ("tail", ["range", "--lower", "495"], lambda v: v >= 495),
        ("null", ["null"], None),
        ("numeric", ["range"], lambda _: True),
        ("empty", ["range", "--lower", str(2**31 - 1), "--exclude-lower"], lambda _: False),
    ]
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        for index, column in [("PrimaryKey", 0), ("CategoryDesc", 1)]:
            reference = subprocess.check_output([
                "java", "-cp", str(args.jar), "docs/testing/IndexRowsOracle.java", str(path),
                "Indexed", index, "ID", "Category",
            ], text=True).splitlines()
            for name, selection, predicate in cases:
                expected = []
                for row in reference:
                    value = row.split("\t")[column]
                    include = (value == "NULL") if predicate is None else (
                        value != "NULL" and predicate(int(value))
                    )
                    if include:
                        expected.append(row)
                actual = subprocess.check_output([
                    str(args.example), str(path), "--index", index, *selection,
                ], text=True).splitlines()
                if actual != expected:
                    raise RuntimeError(f"range disagreement: {format} {index} {name}")
                digest = hashlib.sha256("".join(row + "\n" for row in actual).encode()).hexdigest()
                print(format, index, name, len(actual), digest, flush=True)


if __name__ == "__main__":
    main()
