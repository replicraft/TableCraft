"""Independent U+0020 trimming of complete raw String units.

Python list prefix/suffix removal supplies expected bytes. Native getters supply
rows only; TableCraft evaluates SQL and native Access qualification is unrun.
"""

import argparse
from itertools import product
from pathlib import Path

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import expression
from compare_raw_string_predicates import query

TRIM_FUNCTIONS = ["LTrim", "RTrim", "Trim"]
PROJECTIONS_PER_BATCH = 6


def trimmed(function, units):
    result = list(units)
    if function != "RTrim":
        while result and result[0] == 32:
            result.pop(0)
    if function != "LTrim":
        while result and result[-1] == 32:
            result.pop()
    return result


def vectors():
    for width in range(4):
        for values in product([0, 32, 65, 160, 0xd800, 0xdc00], repeat=width):
            yield list(values)
    yield from [
        [32, 32, 0xd83d, 0xde00, 32],
        [32, 9, 0xd800, 9, 32],
        [32, 0x2000, 0xd800, 0x2000, 32],
        [32, 32, 32, 32, 32],
        [0xd800, 0xdc00, 32, 32],
        [32, 32, 0xd800, 0xdc00],
    ]


def source_free(args):
    controls = 0
    for mode in ["eval", "run"]:
        projections = []
        expected = []
        for units in vectors():
            for function in TRIM_FUNCTIONS:
                result = trimmed(function, units)
                call = f"{function}({expression(units)})"
                projections.extend([f"LenB({call})", f"StrComp({call},{expression(result)},0)"])
                expected.extend([str(len(result) * 2), "0"])
                if len(projections) == PROJECTIONS_PER_BATCH:
                    controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected])
                    projections = []
                    expected = []
        if projections:
            controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected])
        controls += query(args.cli, mode,
                          "SELECT AscW(Trim(MidB(ChrW(8192)&ChrW(0)&ChrW(8192)&ChrW(0),2,6))),Trim(NULL),LenB(Trim(Replace(' '&ChrW(55296)&' ','z','')))",
                          [["0", "NULL", "2"]])
        for function in TRIM_FUNCTIONS:
            for sql in [
                f"SELECT {function}(ChrW(55296))",
                f"SELECT LenB({function}(ChrB(32)))",
                f"SELECT LenB({function}(ChrW(55296))),1/0",
                f"SELECT {function}(CVErr(11))",
            ]:
                controls += query(args.cli, mode, sql, None)
    return controls


def native_units(field):
    if field == "N":
        return []
    assert field.startswith("T"), field
    raw = bytes.fromhex(field[1:]).decode("utf-8").encode("utf-16-le")
    return [int.from_bytes(raw[offset:offset + 2], "little") for offset in range(0, len(raw), 2)]


def native_controls(args):
    controls = 0
    for profile, suffix in PROFILES:
        path = args.directory / f"Policies-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", path, "PolicyProbe")
        expected = []
        for line in oracle.splitlines():
            fields = line.split("\t")
            assert len(fields) == 4 and fields[0] == "PolicyProbe" and fields[1].startswith("I32:")
            legacy = [32, 0xd800, *native_units(fields[2]), 32]
            modern = [32, 0xd800, *native_units(fields[3]), 32]
            expected.append([
                fields[1][4:],
                str(len(trimmed("LTrim", modern)) * 2),
                str(len(trimmed("RTrim", legacy)) * 2),
                str(len(trimmed("Trim", modern)) * 2),
            ])
        expected.sort(key=lambda row: int(row[0]))
        assert [row[0] for row in expected] == [str(index) for index in range(1, 7)]
        sql = (
            "SELECT ID,LenB(LTrim(' '&ChrW(55296)&ModernText&' ')),"
            "LenB(RTrim(' '&ChrW(55296)&LegacyText&' ')),"
            "LenB(Trim(' '&ChrW(55296)&ModernText&' ')) FROM PolicyProbe ORDER BY ID"
        )
        for policy in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                controls += query(args.cli, "run", sql, expected,
                                  ["--database", str(path), "--collation", policy, *index])
        assert digest(path) == before
        print(f"{profile}: raw trimming native-source controls; unchanged {before}")
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
    print(f"Passed {controls} independent raw trimming controls; native Access unrun.")


if __name__ == "__main__":
    main()
