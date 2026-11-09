"""Independent retained String extrema over General keys and inert native IDs.

Python selects first equal representatives from reviewed short keys. Optional
Jackcess getters provide stored IDs only; TableCraft alone evaluates SQL.
"""

import argparse
from pathlib import Path
import random

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest
from compare_native_collations import JAR_SHA256
from compare_raw_string_order import expression, read_vectors
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
        operand = "ChrW(55296)&ChrW(96+ID)"

        for collation in ["database", "general-legacy", "general-2010"]:
            for index in [[], ["--index", "PolicyProbe", "PrimaryKey"]]:
                options = ["--database", str(path), "--collation", collation, *index]
                for criterion, selected in [
                    ("ID>0", ids),
                    ("ID>=3 AND ID<=5", [value for value in ids if 3 <= value <= 5]),
                    ("ID=0", []),
                ]:
                    low = min(selected) if selected else 1
                    high = max(selected) if selected else 1
                    low_expression = f"ChrW(55296)&ChrW({96 + low})"
                    high_expression = f"ChrW(55296)&ChrW({96 + high})"
                    expected = [["4", "0", "4", "0"]] if selected else [["NULL"] * 4]
                    sql = (
                        f"SELECT LenB(Min({operand})),StrComp(Min({operand}),{low_expression},0),"
                        f"LenB(Max({operand})),StrComp(Max({operand}),{high_expression},0) "
                        f"FROM PolicyProbe WHERE {criterion}"
                    )
                    controls += query(args.cli, "run", sql, expected, options)
                    minimum = f"DMin('{operand}','PolicyProbe','{criterion}')"
                    maximum = f"DMax('{operand}','PolicyProbe','{criterion}')"
                    sql = (
                        f"SELECT LenB({minimum}),StrComp({minimum},{low_expression},0),"
                        f"LenB({maximum}),StrComp({maximum},{high_expression},0)"
                    )
                    controls += query(args.cli, "run", sql, expected, options)

        assert digest(path) == before
        print(f"{profile}: raw aggregate/domain controls; unchanged {before}")
    return controls


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if (args.jar is None) != (args.directory is None):
        parser.error("--jar and --directory must be supplied together")
    rng = random.Random(20261024)
    cases = {"0": [], "1": []}
    for family, units, key in read_vectors():
        cases[family].append((units, key))
    controls = 0

    for family, collation in [("0", "general-legacy"), ("1", "general-2010")]:
        options = ["--collation", collation]
        orders = [cases[family], list(reversed(cases[family]))]
        for _ in range(6):
            orders.append(rng.sample(cases[family], len(cases[family])))
        for records in orders:
            minimum = min(records, key=lambda record: record[1])
            maximum = max(records, key=lambda record: record[1])
            selects = [f"SELECT {index} AS ID,{expression(units)} AS S"
                       for index, (units, _) in enumerate(records, 1)]
            selects.insert(0, "SELECT 0 AS ID,NULL AS S")
            source = "(" + " UNION ALL ".join(selects) + ") AS Q"
            sql = (
                f"SELECT LenB(Min(S)),StrComp(Min(S),{expression(minimum[0])},0),"
                f"LenB(Max(S)),StrComp(Max(S),{expression(maximum[0])},0) FROM {source}"
            )
            expected = [[str(2 * len(minimum[0])), "0", str(2 * len(maximum[0])), "0"]]
            controls += query(args.cli, "run", sql, expected, options)

            groups = {}
            for index, record in enumerate(records, 1):
                groups.setdefault(index % 3, []).append(record)
            expected = []
            for group, members in sorted(groups.items()):
                low = min(members, key=lambda record: record[1])
                high = max(members, key=lambda record: record[1])
                expected.append([str(group), str(2 * len(low[0])), str(2 * len(high[0]))])
            sql = f"SELECT ID MOD 3 AS G,LenB(Min(S)),LenB(Max(S)) FROM {source} GROUP BY ID MOD 3 ORDER BY ID MOD 3"
            controls += query(args.cli, "run", sql, expected, options)

    for sql, expected in [
        ("SELECT LenB(Min(ChrB(65))),LenB(Max(ChrW(55296)))", [["1", "2"]]),
        ("SELECT Min(S),Max(S) FROM (SELECT NULL AS S UNION ALL SELECT NULL) AS Q", [["NULL", "NULL"]]),
    ]:
        controls += query(args.cli, "run", sql, expected)
    for sql in [
        "SELECT Min(ChrW(55296))",
        "SELECT LenB(Min(S)) FROM (SELECT ChrB(65) AS S UNION ALL SELECT ChrB(65)) AS Q",
        "SELECT LenB(Min(ChrW(55296/(2-ID)))) FROM (SELECT 1 AS ID UNION ALL SELECT 2) AS Q",
    ]:
        controls += query(args.cli, "run", sql, None, ["--collation", "general-2010"])
    if args.directory is not None:
        controls += native_controls(args)
    print(f"Passed {controls} independent raw String extrema controls; native Access unrun.")


if __name__ == "__main__":
    main()
