"""Independent first-unit String construction under explicit count policies.

Python integer-list repetition supplies exact expected UTF-16 bytes. Independent
native getters supply rows only; TableCraft alone evaluates SQL.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
from pathlib import Path
import random

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import expression
from compare_raw_string_predicates import query

PROJECTIONS_PER_BATCH = 6


def checks(call, units):
    return [f"LenB({call})", f"StrComp({call},{expression(units)},0)"], [str(len(units) * 2), "0"]


def source_free(args):
    controls = 0
    rng = random.Random(20261024)
    units = [0, 9, 32, 65, 128, 160, 0x2000, 0xd7ff, 0xd800, 0xdbff, 0xdc00, 0xdfff, 0xe000, 0xffff]
    units += [rng.randrange(65536) for _ in range(24)]
    for mode in ["eval", "run"]:
        projections = []
        expected = []
        for unit in units:
            for tail in [[], [0xd800], [0xd83d, 0xde00]]:
                for count in [0, 1, 2, 5]:
                    call = f"String({count},{expression([unit, *tail])})"
                    expressions, values = checks(call, [unit] * count)
                    projections.extend(expressions)
                    expected.extend(values)
                    if len(projections) == PROJECTIONS_PER_BATCH:
                        controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected])
                        projections = []
                        expected = []
        if projections:
            controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected])
        for profile, text in [("en-us", "2.5"), ("de-de", "2,5")]:
            count = int(Decimal(text.replace(",", ".")).to_integral_value(rounding=ROUND_HALF_EVEN))
            call = f"String('{text}',ChrW(55296))"
            expressions, values = checks(call, [0xd800] * count)
            controls += query(args.cli, mode, "SELECT " + ",".join(expressions), [values], ["--number-format", profile])
        controls += query(args.cli, mode,
                          "SELECT LenB(String(2,MidB('😀',2,2))),String(NULL,ChrB(65)),String(ChrB(65),NULL),VarType(String(2,'😀'))",
                          [["4", "NULL", "NULL", "8"]])
        for sql in [
            "SELECT String(1,'😀')",
            "SELECT LenB(String(0,ChrB(65)))",
            "SELECT LenB(String(-1,ChrB(65)))",
            "SELECT LenB(String(0,LeftB('',0)))",
            "SELECT LenB(String(0,CVErr(11)))",
            "SELECT LenB(String(2147483647,ChrW(55296)))",
            "SELECT String(NULL,1/0)",
            "SELECT LenB(String(2,ChrW(55296))),1/0",
        ]:
            controls += query(args.cli, mode, sql, None)
    return controls


def native_units(field):
    if field == "N":
        return []
    assert field.startswith("T"), field
    data = bytes.fromhex(field[1:]).decode("utf-8").encode("utf-16-le")
    return [int.from_bytes(data[offset:offset + 2], "little") for offset in range(0, len(data), 2)]


def native_controls(args):
    controls = 0
    for profile, suffix in PROFILES:
        path = args.directory / f"Policies-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", path, "PolicyProbe")
        rows = []
        for line in oracle.splitlines():
            fields = line.split("\t")
            assert len(fields) == 4 and fields[0] == "PolicyProbe" and fields[1].startswith("I32:")
            ident = int(fields[1][4:])
            legacy = (native_units(fields[2]) + [0xd800])[0]
            modern = (native_units(fields[3]) + [0xd800])[0]
            rows.append((ident, legacy, modern))
        assert sorted(row[0] for row in rows) == list(range(1, 7))
        for policy in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                for ident, legacy, modern in rows:
                    sql = (
                        "SELECT ID,LenB(String(ID,ChrW(55296))),"
                        f"StrComp(String(ID,LegacyText&ChrW(55296)),{expression([legacy] * ident)},0),"
                        f"StrComp(String(ID,ModernText&ChrW(55296)),{expression([modern] * ident)},0) "
                        f"FROM PolicyProbe WHERE ID={ident}"
                    )
                    controls += query(args.cli, "run", sql, [[str(ident), str(ident * 2), "0", "0"]],
                                      ["--database", str(path), "--collation", policy, *index])
        assert digest(path) == before
        print(f"{profile}: raw repetition native-source controls; unchanged {before}")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if (args.jar is None) != (args.directory is None):
        parser.error("--jar and --directory must be supplied together")
    controls = source_free(args)
    if args.jar is not None:
        before = digest(args.jar)
        if before != JAR_SHA256:
            raise ValueError("unexpected independent artifact")
        controls += native_controls(args)
        assert digest(args.jar) == before
    print(f"Passed {controls} independent raw repetition controls; native Access unrun.")


if __name__ == "__main__":
    main()
