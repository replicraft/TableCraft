"""Compare raw String SQL predicates with registered independent General keys.

The reviewed short key vectors supply expected order, including isolated units.
They qualify the explicit provisional model, not native Microsoft Access SQL.
"""

import argparse
from pathlib import Path

from compare_raw_string_predicates import query

PROJECTIONS_PER_BATCH = 12


def expression(units):
    if not units:
        return "LeftB('',0)"
    if len(units) == 1:
        return f"ChrW({units[0]})"
    middle = len(units) // 2
    return f"({expression(units[:middle])}&{expression(units[middle:])})"


def read_vectors():
    vectors = Path(__file__).resolve().parents[2] / "crates/engines/tablecraft-ace/src/collation/key/tests/vectors.tsv"
    for line in vectors.read_text(encoding="ascii").splitlines():
        family, raw, key, _ = line.split("\t")
        if raw != "-" and len(raw) > 16:
            continue
        units = [] if raw == "-" else [int(raw[at:at + 4], 16) for at in range(0, len(raw), 4)]
        yield family, units, bytes.fromhex(key)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    cases = {"0": [], "1": []}
    for family, units, key in read_vectors():
        cases[family].append((expression(units), key))

    controls = 0
    for family, collation in [("0", "general-legacy"), ("1", "general-2010")]:
        options = ["--collation", collation]
        expressions = []
        expected = []
        for left, left_key in cases[family]:
            for right, right_key in cases[family]:
                order = (left_key > right_key) - (left_key < right_key)
                for operator, truth in [
                    ("=", order == 0), ("<>", order != 0),
                    ("<", order < 0), ("<=", order <= 0),
                    (">", order > 0), (">=", order >= 0),
                ]:
                    expressions.append(f"{left}{operator}{right}")
                    expected.append("TRUE" if truth else "FALSE")
                    if len(expressions) == PROJECTIONS_PER_BATCH:
                        controls += query(args.cli, "run", "SELECT " + ",".join(expressions), [expected], options)
                        expressions = []
                        expected = []
        if expressions:
            controls += query(args.cli, "run", "SELECT " + ",".join(expressions), [expected], options)

        sql = (
            "SELECT S=ChrW(55296)&'a',S IN(ChrW(55296)&'A'),"
            "S BETWEEN ChrW(55296)&'a' AND ChrW(55296)&'b',"
            "S=ANY(SELECT ChrW(55296)&'a'),S=ALL(SELECT ChrW(55296)&'A') "
            "FROM (SELECT ChrW(55296)&'A ' AS S) AS Q"
        )
        controls += query(args.cli, "run", sql, [["TRUE"] * 5], options)
        for sql in [
            "SELECT ChrB(65)=ChrW(55296)",
            "SELECT ChrW(55296)=ChrW(55296),1/0",
            "SELECT DISTINCT ChrW(55296)",
        ]:
            controls += query(args.cli, "run", sql, None, options)

    controls += query(args.cli, "run", "SELECT ChrW(55296)=ChrW(55296)", None,
                      ["--collation", "ordinal"])
    print(f"Passed {controls} independent raw String order controls; native Access unrun.")


if __name__ == "__main__":
    main()
