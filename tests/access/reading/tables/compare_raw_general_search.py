"""Independent short-span General searches over complete raw UTF-16 Strings.

The reviewed external encoder supplies keys to the existing original Java model.
No native Microsoft Access query execution or broader locale support is claimed.
"""

import argparse
import os
from pathlib import Path
import tempfile

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import PROJECTIONS_PER_BATCH, expression
from compare_raw_string_predicates import query


def units(encoded):
    return [] if encoded == "-" else [int(encoded[offset:offset + 4], 16)
                                    for offset in range(0, len(encoded), 4)]


def source_free(args, classpath):
    controls = 0
    for version, policy in [(0, "general-legacy"), (1, "general-2010")]:
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", classpath,
                     "com.healthmarketscience.jackcess.impl.RawGeneralSearchOracle", version)
        expressions = []
        expected = []
        for row in oracle.splitlines():
            kind, haystack, needle, start, position = row.split("\t")
            haystack, needle = expression(units(haystack)), expression(units(needle))
            call = (f"InStr({start},{haystack},{needle},2)" if kind == "F"
                    else f"InStrRev({haystack},{needle},{start},2)")
            expressions.append(call)
            expected.append(position)
            if len(expressions) == PROJECTIONS_PER_BATCH:
                controls += query(args.cli, "run", "SELECT " + ",".join(expressions),
                                  [expected], ["--collation", policy])
                expressions = []
                expected = []
        if expressions:
            controls += query(args.cli, "run", "SELECT " + ",".join(expressions),
                              [expected], ["--collation", policy])
        controls += query(args.cli, "run",
                          "SELECT InStr(ChrW(55296),ChrW(55296)),InStr(1,MidB('😀',2,2),ChrW(216),2),InStr(1,ChrB(65),NULL,2)",
                          [["1", "1", "NULL"]], ["--collation", policy])
        for sql in [
            "SELECT InStr(0,ChrW(55296),NULL,2)",
            "SELECT InStr(1,ChrB(65),'',2)",
            "SELECT InStr(1,ChrW(55296),ChrW(55296),1)",
            "SELECT InStr(1,ChrW(55296),ChrW(55296),1033)",
            "SELECT InStrRev(ChrW(55296),ChrW(55296),-1,2),1/0",
        ]:
            controls += query(args.cli, "run", sql, None, ["--collation", policy])
        print(f"{policy}: completed {controls} cumulative raw General controls", flush=True)
    return controls


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
            assert fields[0] == "PolicyProbe" and fields[1].startswith("I32:")
            expected.append([fields[1][4:], "1", "1", "1"])
        expected.sort(key=lambda row: int(row[0]))
        assert [row[0] for row in expected] == [str(index) for index in range(1, 7)]
        sql = (
            "SELECT ID,InStr(1,ChrW(55296)&ModernText,ChrW(55296),2),"
            "InStrRev(ChrW(55296)&LegacyText,ChrW(55296),1,2),"
            "InStr(ChrW(55296)&ModernText,ChrW(55296)) FROM PolicyProbe ORDER BY ID"
        )
        for policy in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                controls += query(args.cli, "run", sql, expected,
                                  ["--database", str(path), "--collation", policy, *index])
        assert digest(path) == before
        print(f"{profile}: raw General native-source controls; unchanged {before}")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    before = digest(args.jar)
    if before != JAR_SHA256:
        raise ValueError("unexpected independent artifact")
    with tempfile.TemporaryDirectory(prefix="tablecraft-raw-general-search-") as classes:
        run("javac", "-cp", args.jar, "-d", classes,
            RECIPES / "GeneralSearchOracle.java", RECIPES / "RawGeneralSearchOracle.java")
        controls = source_free(args, os.pathsep.join([classes, str(args.jar)]))
    if args.directory is not None:
        controls += native_controls(args)
    assert digest(args.jar) == before
    print(f"Passed {controls} independent raw General search controls; native Access unrun.")


if __name__ == "__main__":
    main()
