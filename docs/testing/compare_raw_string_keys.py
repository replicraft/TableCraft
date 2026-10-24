"""Independent raw String key consumers using registered short General vectors.

Python groups and sorts the reviewed keys; TableCraft alone evaluates SQL.
The provisional General model and stable representatives are not native Access
SQL qualification. Original fixture bytes are neither changed nor generated.
"""

import argparse
from pathlib import Path

from compare_raw_string_order import expression, read_vectors
from compare_raw_string_predicates import query


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    families = {"0": [], "1": []}
    for family, units, key in read_vectors():
        families[family].append((units, key))
    controls = 0

    for family, collation in [("0", "general-legacy"), ("1", "general-2010")]:
        options = ["--collation", collation]
        cases = families[family]
        selects = [f"SELECT {index} AS ID,{expression(units)} AS S"
                   for index, (units, _) in enumerate(cases, 1)]
        null_id = len(selects) + 1
        selects.append(f"SELECT {null_id},NULL")
        source = "(" + " UNION ALL ".join(selects) + ") AS Q"
        string_arms = [f"SELECT {expression(units)} AS S" for units, _ in cases]
        string_arms.append("SELECT NULL")
        string_source = " UNION ALL ".join(string_arms)
        records = [(index, key, len(units) * 2)
                   for index, (units, key) in enumerate(cases, 1)]
        records.append((null_id, None, None))
        groups = {}
        for record in records:
            groups.setdefault(record[1], []).append(record)

        for descending in [False, True]:
            expected = sorted(records, key=lambda row: (row[1] is not None, row[1] or b""),
                              reverse=descending)
            sql = f"SELECT ID FROM {source} ORDER BY S" + (" DESC" if descending else "")
            controls += query(args.cli, "run", sql, [[str(row[0])] for row in expected], options)

        expected_groups = [
            [str(group[0][0]), str(len(group)), "NULL" if group[0][2] is None else str(group[0][2])]
            for group in groups.values()
        ]
        sql = f"SELECT Min(ID),Count(*),LenB(S) FROM {source} GROUP BY S ORDER BY Min(ID)"
        controls += query(args.cli, "run", sql, expected_groups, options)

        sql = f"SELECT Count(*) FROM (SELECT DISTINCT S FROM {source}) AS D"
        controls += query(args.cli, "run", sql, [[str(len(groups))]], options)
        sql = f"SELECT Count(*) FROM ({string_source} UNION SELECT ChrW(55296)) AS D"
        controls += query(args.cli, "run", sql, [[str(len(groups))]], options)

        minimum = min(key for key in groups if key is not None)
        sql = f"SELECT TOP 1 ID FROM {source} WHERE S IS NOT NULL ORDER BY S"
        controls += query(args.cli, "run", sql, [[str(row[0])] for row in groups[minimum]], options)

        sql = (f"SELECT DISTINCTROW LenB(D.S) FROM ({string_source}) AS D "
               "INNER JOIN (SELECT 1 AS N UNION ALL SELECT 2) AS J ON True ORDER BY 1")
        lengths = sorted((group[0][2] for group in groups.values()), key=lambda length: (length is not None, length or 0))
        expected = [["NULL" if length is None else str(length)] for length in lengths]
        controls += query(args.cli, "run", sql, expected, options)

        for sql in [
            "SELECT Count(*) FROM (SELECT DISTINCT ChrW(55296) AS S) AS Q",
            "SELECT Count(*) FROM (SELECT DISTINCT ChrB(65) AS S) AS Q",
            "SELECT Count(*) FROM (SELECT DISTINCT CVErr(1) AS S) AS Q",
        ]:
            controls += query(args.cli, "run", sql, [["1"]], options)
        for sql in [
            "SELECT DISTINCT ChrW(55296)",
            "SELECT Count(*) FROM (SELECT ChrB(65) AS S UNION SELECT ChrB(65)) AS Q",
            "SELECT TOP 1 ID FROM (SELECT 1 AS ID,ChrW(55296) AS S UNION ALL SELECT 2,ChrW(1/0)) AS Q ORDER BY S",
            "SELECT Count(*) FROM (SELECT CVErr(1) AS S UNION SELECT NULL) AS Q",
        ]:
            controls += query(args.cli, "run", sql, None, options)

    controls += query(args.cli, "run", "SELECT Count(*) FROM (SELECT DISTINCT ChrB(65) AS S) AS Q", [["1"]])
    print(f"Passed {controls} independent raw String key controls; native Access unrun.")


if __name__ == "__main__":
    main()
