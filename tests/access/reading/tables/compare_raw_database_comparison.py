"""Raw database-mode StrComp controls under explicit provisional General models.

Reviewed short keys supply order, including isolated units. Optional independent
native getters supply IDs only; no Microsoft Access SQL execution is claimed.
"""

import argparse
from pathlib import Path

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import PROJECTIONS_PER_BATCH, expression, read_vectors
from compare_raw_string_predicates import query


def native_controls(args):
    if digest(args.jar) != JAR_SHA256:
        raise ValueError("unexpected independent artifact")
    controls = 0
    for profile, suffix in PROFILES:
        path = args.directory / f"Policies-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", path, "PolicyProbe")
        ids = []
        for line in oracle.splitlines():
            fields = line.split("\t")
            assert fields[0] == "PolicyProbe" and fields[1].startswith("I32:")
            ids.append(int(fields[1][4:]))
        assert sorted(ids) == list(range(1, 7))
        expected = []
        for ident in sorted(ids):
            order = (ident > 4) - (ident < 4)
            expected.append([str(ident), *([str(order)] * 3)])
        left = "ChrW(55296)&ChrW(96+ID)"
        right = "ChrW(55296)&'d'"
        sql = (
            f"SELECT ID,StrComp({left},{right},2),StrComp({left},{right}),"
            f"StrComp({left},{right},-1) FROM PolicyProbe ORDER BY ID"
        )
        for collation in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                options = ["--database", str(path), "--collation", collation, *index]
                controls += query(args.cli, "run", sql, expected, options)
        assert digest(path) == before
        print(f"{profile}: raw database comparison controls; unchanged {before}")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if (args.jar is None) != (args.directory is None):
        parser.error("--jar and --directory must be supplied together")
    families = {"0": [], "1": []}
    for family, units, key in read_vectors():
        families[family].append((units, key))
    controls = 0

    for family, collation in [("0", "general-legacy"), ("1", "general-2010")]:
        options = ["--collation", collation]
        projections = []
        expected = []
        for left, left_key in families[family]:
            for right, right_key in families[family]:
                order = (left_key > right_key) - (left_key < right_key)
                for suffix in [",2", "", ",-1"]:
                    projections.append(f"StrComp({expression(left)},{expression(right)}{suffix})")
                    expected.append(str(order))
                    if len(projections) == PROJECTIONS_PER_BATCH:
                        controls += query(args.cli, "run", "SELECT " + ",".join(projections), [expected], options)
                        projections = []
                        expected = []
        if projections:
            controls += query(args.cli, "run", "SELECT " + ",".join(projections), [expected], options)

        sql = "SELECT StrComp(LeftB('a ',4),'A',2),StrComp(ChrW(55296),ChrW(55296),'2'),StrComp(ChrB(65),NULL,2)"
        controls += query(args.cli, "run", sql, [["0", "0", "NULL"]],
                          [*options, "--number-format", "en-us"])
        for sql in [
            "SELECT StrComp(ChrB(65),ChrW(65),2)",
            "SELECT StrComp(ChrB(65),NULL,3)",
            "SELECT StrComp(ChrW(65),'a',1)",
            "SELECT StrComp(ChrW(55296),ChrW(55296),2),1/0",
        ]:
            controls += query(args.cli, "run", sql, None, options)
    controls += query(args.cli, "run", "SELECT StrComp(ChrW(55296),ChrW(55296),2)", None,
                      ["--collation", "ordinal"])
    if args.directory is not None:
        controls += native_controls(args)
    print(f"Passed {controls} independent raw database comparison controls; native Access unrun.")


if __name__ == "__main__":
    main()
