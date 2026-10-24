"""Independent complete-unit binary InStr/InStrRev controls.

Python integer-list windows supply positions, including isolated UTF-16 units.
Optional inert native getters supply rows only; Microsoft Access SQL is unrun.
"""

import argparse
from pathlib import Path

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import PROJECTIONS_PER_BATCH, expression
from compare_raw_string_predicates import query


def position(haystack, needle, start, reverse):
    if reverse and start == -1:
        start = len(haystack)
    if not haystack or start > len(haystack):
        return 0
    if not needle:
        return start
    matches = [index + 1 for index in range(len(haystack) - len(needle) + 1)
               if haystack[index:index + len(needle)] == needle
               and (index + len(needle) <= start if reverse else index + 1 >= start)]
    return (matches[-1] if reverse else matches[0]) if matches else 0


def native_controls(args):
    if digest(args.jar) != JAR_SHA256:
        raise ValueError("unexpected independent artifact")
    controls = 0
    for profile, suffix in PROFILES:
        path = args.directory / f"Policies-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", path, "PolicyProbe")
        expected = []
        for line in oracle.splitlines():
            fields = line.split("\t")
            assert fields[0] == "PolicyProbe" and fields[1].startswith("I32:")
            ident = fields[1][4:]
            # Ampersand treats a NULL operand as empty Text; the prefix remains.
            expected.append([ident, "1", "1", "0"])
        expected.sort(key=lambda row: int(row[0]))
        assert [row[0] for row in expected] == [str(index) for index in range(1, 7)]
        sql = (
            "SELECT ID,InStr(1,ChrW(55296)&ModernText,ChrW(55296),0),"
            "InStrRev(ChrW(55296)&LegacyText,ChrW(55296)),"
            "InStr(2,ChrW(55296)&ModernText,ChrW(55296),0) FROM PolicyProbe ORDER BY ID"
        )
        for collation in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                controls += query(args.cli, "run", sql, expected,
                                  ["--database", str(path), "--collation", collation, *index])
        assert digest(path) == before
        print(f"{profile}: raw binary search controls; unchanged {before}")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if (args.jar is None) != (args.directory is None):
        parser.error("--jar and --directory must be supplied together")
    vectors = [[], [0], [65], [0xd800], [0xdc00], [0xffff], [65, 0xd800],
               [0xd800, 65], [0xd800, 65, 0xd800], [0xd83d, 0xde00],
               [65, 65, 65, 66], [0, 65, 0], [32, 0xd800, 32]]
    controls = 0
    for mode in ["eval", "run"]:
        expressions = []
        expected = []
        for haystack in vectors:
            for needle in vectors:
                for start in [1, 2, 3, 5]:
                    for reverse in [False, True]:
                        hay, pattern = expression(haystack), expression(needle)
                        call = (f"InStrRev({hay},{pattern},{start},0)" if reverse
                                else f"InStr({start},{hay},{pattern},0)")
                        expressions.append(call)
                        expected.append(str(position(haystack, needle, start, reverse)))
                        if len(expressions) == PROJECTIONS_PER_BATCH:
                            controls += query(args.cli, mode, "SELECT " + ",".join(expressions), [expected])
                            expressions = []
                            expected = []
        if expressions:
            controls += query(args.cli, mode, "SELECT " + ",".join(expressions), [expected])
        for haystack in vectors:
            for needle in vectors:
                sql = f"SELECT InStrRev({expression(haystack)},{expression(needle)})"
                controls += query(args.cli, mode, sql, [[str(position(haystack, needle, -1, True))]])
        controls += query(args.cli, mode,
                          "SELECT InStr(1,ChrB(65),NULL,0),InStrRev(NULL,ChrB(65)),InStr(1,MidB('😀',2,2),ChrW(216),0)",
                          [["NULL", "NULL", "1"]])
        for sql in [
            "SELECT InStr(0,ChrW(55296),NULL,0)",
            "SELECT InStr(1,ChrB(65),'',0)",
            "SELECT InStrRev('',ChrB(65))",
            "SELECT InStr(1,ChrW(55296),ChrW(55296),1033)",
            "SELECT InStrRev(ChrW(55296),ChrW(55296)),1/0",
        ]:
            controls += query(args.cli, mode, sql, None)
    if args.directory is not None:
        controls += native_controls(args)
    print(f"Passed {controls} independent raw binary search controls; native Access unrun.")


if __name__ == "__main__":
    main()
