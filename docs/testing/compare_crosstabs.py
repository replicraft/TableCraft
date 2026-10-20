"""Compare native-backed crosstabs with an independent model of reference rows.

Uses inert Jackcess cursors, not its expression evaluator or an Access SQL oracle.
"""
import argparse
from pathlib import Path

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "database"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    before = digest(args.database)
    controls = 0
    for index in [None, "Number"]:
        cursor = ["--index", "ROULETTE_TB", index] if index else ["ROULETTE_TB"]
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", args.database, *cursor)
        rows = [line.split("\t") for line in tagged.splitlines()]
        assert len(rows) == 38
        def width(row):
            return len(bytes.fromhex(row[2][1:]).decode("utf-8").encode("utf-16-le")) // 2
        dynamic = sorted({width(row) for row in rows})
        for aggregate, headings, filtered, having in [
            ("Count(*)", dynamic, False, False),
            ("Count(*)", [2, 1, 3], False, False),
            ("Count(*)", dynamic, True, False),
            ("Sum(ORDR_NBR)", dynamic, False, True),
            ("First(NBR_VALUE)", dynamic, False, False),
        ]:
            groups = {}
            for row in rows:
                if not filtered or width(row) == 1:
                    groups.setdefault(int(row[3][4:]), []).append(row)
            expected = "COLOR_ID\tTotal\t" + "\t".join(map(str, headings)) + "\n"
            for color, members in sorted(groups.items()):
                if having and len(members) <= 10:
                    continue
                cells = []
                for heading in headings:
                    cell = [row for row in members if width(row) == heading]
                    if not cell:
                        cells.append("NULL")
                    elif aggregate == "Count(*)":
                        cells.append(str(len(cell)))
                    elif aggregate.startswith("Sum"):
                        cells.append(str(sum(int(row[1][4:]) for row in cell)))
                    else:
                        cells.append(display(cell[0][2]))
                expected += f"{color}\t{len(members)}\t" + "\t".join(cells) + "\n"
            sql = f"TRANSFORM {aggregate} SELECT COLOR_ID,Count(*) AS Total FROM ROULETTE_TB"
            if filtered:
                sql += " WHERE Len(NBR_VALUE)=1"
            sql += " GROUP BY COLOR_ID"
            if having:
                sql += " HAVING Count(*)>10"
            sql += " ORDER BY COLOR_ID PIVOT Len(NBR_VALUE)"
            if headings == [2, 1, 3]:
                sql += " IN (2,1,3)"
            options = ["--database", args.database]
            if index:
                options += ["--index", "ROULETTE_TB", index]
            assert run(args.cli, "query", "run", sql, *options) == expected, (index, sql)
            controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} crosstab reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
