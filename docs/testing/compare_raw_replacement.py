"""Independent raw String replacement controls for binary and General profiles.

Python supplies nonoverlapping unit replacement; the reviewed external key encoder
supplies independent General spans. Native getters provide data only, not SQL.
"""

import argparse
from itertools import product
import os
from pathlib import Path
import tempfile

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_general_search import units
from compare_raw_string_order import expression
from compare_raw_string_predicates import query

REPLACEMENT_PROJECTIONS_PER_BATCH = 6


def binary(source, find, replacement, start, count):
    remaining = source[start - 1:]
    if not find or count == 0:
        return remaining
    result = []
    while remaining:
        if remaining[:len(find)] == find and count != 0:
            result.extend(replacement)
            remaining = remaining[len(find):]
            if count > 0:
                count -= 1
        else:
            result.append(remaining[0])
            remaining = remaining[1:]
    return result


def observe(call, expected):
    return [f"LenB({call})", f"StrComp({call},{expression(expected)},0)"], [str(len(expected) * 2), "0"]


def batches(args, mode, cases, options=()):
    controls = 0
    projections = []
    expected = []
    for call, result in cases:
        expressions, values = observe(call, result)
        projections.extend(expressions)
        expected.extend(values)
        if len(projections) == REPLACEMENT_PROJECTIONS_PER_BATCH:
            controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected], options)
            projections = []
            expected = []
    if projections:
        controls += query(args.cli, mode, "SELECT " + ",".join(projections), [expected], options)
    return controls


def binary_cases():
    sources = [[], [0xd800], [0xdc00], [0xd800, 65, 0xd800], [0xd83d, 0xde00],
               [65, 65, 65, 66], [0, 0xd800, 32], [65, 0xdfff]]
    finds = [[], [65], [0xd800], [0xde00], [65, 65]]
    replacements = [[], [66], [0xdfff], [0xd800, 0xdc00]]
    for source, find, replacement, start, count in product(sources, finds, replacements, [1, 2, 5], [-1, 0, 1, 2]):
        call = f"Replace({expression(source)},{expression(find)},{expression(replacement)},{start},{count},0)"
        yield call, binary(source, find, replacement, start, count)


def general_cases(output):
    for line in output.splitlines():
        source, find, replacement, start, count, expected = line.split("\t")
        call = f"Replace({expression(units(source))},{expression(units(find))},{expression(units(replacement))},{start},{count},2)"
        yield call, units(expected)


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
            ident = fields[1][4:]
            expected.append([ident, "2", "0", "0"])
        expected.sort(key=lambda row: int(row[0]))
        assert [row[0] for row in expected] == [str(index) for index in range(1, 7)]
        sql = (
            "SELECT ID,LenB(Replace(ChrW(55296)&ChrW(96+ID),ChrW(96+ID),'',1,-1,0)),"
            "StrComp(Replace(ChrW(55296)&ModernText,ChrW(55296),ChrW(57343),1,1,2),"
            "ChrW(57343)&ModernText,0),"
            "StrComp(Replace(ChrW(55296)&LegacyText,ChrW(55296),ChrW(57343),1,1,2),"
            "ChrW(57343)&LegacyText,0) FROM PolicyProbe ORDER BY ID"
        )
        for policy in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                controls += query(args.cli, "run", sql, expected,
                                  ["--database", str(path), "--collation", policy, *index])
        assert digest(path) == before
        print(f"{profile}: raw replacement native-source controls; unchanged {before}")
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
    controls = 0
    for mode in ["eval", "run"]:
        controls += batches(args, mode, binary_cases())
        controls += query(args.cli, mode, "SELECT LenB(Replace('😀','z','',2,0)),AscW(Replace('😀',ChrW(55357),''))", [["2", "-8704"]])
        for sql in [
            "SELECT Replace(ChrW(55296),'a','b')",
            "SELECT LenB(Replace(ChrB(65),'a','b'))",
            "SELECT LenB(Replace(ChrW(55296),'a','b')),1/0",
            "SELECT LenB(Replace(NULL,ChrW(55296),''))",
        ]:
            controls += query(args.cli, mode, sql, None)
    with tempfile.TemporaryDirectory(prefix="tablecraft-raw-replacement-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, RECIPES / "GeneralSearchOracle.java",
            RECIPES / "RawGeneralSearchOracle.java", RECIPES / "RawGeneralReplacementOracle.java")
        classpath = os.pathsep.join([classes, str(args.jar)])
        for version, policy in [(0, "general-legacy"), (1, "general-2010")]:
            oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", classpath,
                         "com.healthmarketscience.jackcess.impl.RawGeneralReplacementOracle", version)
            controls += batches(args, "run", general_cases(oracle), ["--collation", policy])
    if args.directory is not None:
        controls += native_controls(args)
    assert digest(args.jar) == before
    print(f"Passed {controls} independent raw replacement controls; native Access unrun.")


if __name__ == "__main__":
    main()
