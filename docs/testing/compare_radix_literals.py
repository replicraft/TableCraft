"""Independent integer/range/type models and inert native-field radix controls.

The reference reads stored fields only. Only TableCraft executes SQL; models do
not qualify Access literal typing, optimizer choices or native error precedence.
"""

import argparse
import hashlib
from pathlib import Path
import random

from compare_arithmetic_promotion import check
from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def expected(bits):
    width = 16 if bits <= 65535 else 32
    value = bits if bits < 2 ** (width - 1) else bits - 2 ** width
    return value, 2 if width == 16 else 3


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261022)
    values = [0, 1, 32767, 32768, 65535, 65536, 2147483647, 2147483648, 4294967295]
    values += [rng.randrange(0, 2 ** 32) for _ in range(32)]
    count = 0
    for bits in values:
        for literal in [f"&H{bits:X}", f"&O{bits:o}"]:
            value, subtype = expected(bits)
            negative_type = (3 if value == -32768 else 2) if subtype == 2 else (5 if value == -(2 ** 31) else 3)
            for mode in ["eval", "run"]:
                check(execute(args.cli, mode, f"SELECT {literal},VarType({literal}),-{literal},VarType(-{literal})"), [[value, subtype, -value, negative_type]])
                count += 4
    for mode in ["eval", "run"]:
        for literal in ["&H100000000", "&O40000000000", "&HGG", "&O8", "&H" + "0" * 127, "& HFF", "&\nHFF", "&[HFF]", "&/*gap*/HFF"]:
            execute(args.cli, mode, "SELECT 1," + literal, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        expected_rows = [[value, value + 1, 3, value % 2] for value in identities]
        check(execute(args.cli, "run", "SELECT ID,ID+&H1,VarType(ID+&H1),ID Mod &O2 FROM ScalarSamples ORDER BY ID", database), expected_rows)
        check(execute(args.cli, "run", "SELECT ID FROM ScalarSamples WHERE ID=&H1", database), [[value] for value in identities if value == 1])
        sql = "TRANSFORM Sum(ID) SELECT &H1 AS G FROM ScalarSamples GROUP BY &H1 PIVOT ID IN (&O1,&H2,&H3)"
        check(execute(args.cli, "run", sql, database), [[1, *[sum(value for value in identities if value == heading) if heading in identities else None for heading in [1, 2, 3]]]])
        count += 4 * len(identities) + sum(value == 1 for value in identities) + 4
        assert database.read_bytes() == original
        print(database.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {count} independent signed-width/type/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
