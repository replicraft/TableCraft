"""Compare parent membership criteria with independent nullable set folds.

Jackcess supplies original stored parent IDs and values. These explicit three-
valued models are not a Microsoft Access SQL or NULL/error-order oracle.
"""
import argparse
from pathlib import Path

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest
from compare_multivalue_queries import rejected


def membership(values, chosen):
    if any(value == chosen for value in values):
        return True
    return None if "N" in values else False


def negate(value):
    return None if value is None else not value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2007", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    controls = 0
    for path in [args.ace2007, args.ace2016, args.ace2019]:
        before = digest(path)
        options = ["--database", path, "--multivalues", "--native-bytes", "67108864"]
        text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "MultivalueOracle.java", path, "Multivalues", "Ints", "ID")
        rows = [line.split("\t") for line in text.splitlines()]
        cases = [
            ("Ints.Value=0", lambda values: membership(values, "I32:0")),
            ("NOT Ints.Value=0", lambda values: negate(membership(values, "I32:0"))),
            ("Ints.Value<>0", lambda values: negate(membership(values, "I32:0"))),
            ("Ints.Value=0 AND Ints.Value=1", lambda values: membership(values, "I32:0") is True and membership(values, "I32:1") is True),
            ("Ints.Value IN (0,1)", lambda values: membership(values, "I32:0") is True or membership(values, "I32:1") is True),
            ("Ints.Value IS NULL", lambda values: "N" in values),
        ]
        for criteria, model in cases:
            selected = [row for row in rows if model(row[4:]) is True]
            expected = "ID\n" + "".join(display(row[3]) + "\n" for row in selected)
            sql = "SELECT ID FROM Multivalues WHERE " + criteria + " ORDER BY ID"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
            sql = "SELECT Count(*) AS N FROM Multivalues WHERE " + criteria
            assert run(args.cli, "query", "run", sql, *options) == f"N\n{len(selected)}\n", (path, sql)
            controls += 1
        # Joined parents stay at parent-pair cardinality, using only independent
        # inert integer lists and explicit nullable membership folds.
        identities = {int(display(row[3])): row for row in rows}
        pairs = [(a, b) for a in sorted(identities) for b in sorted(identities)]
        def grid(pairs):
            return "AID\tBID\n" + "".join(("NULL" if a is None else str(a)) + "\t" + ("NULL" if b is None else str(b)) + "\n" for a, b in pairs)
        positive = [(a, b) for a, b in pairs if membership(identities[a][4:], f"I32:{b}") is True]
        cross = "SELECT A.ID AS AID,B.ID AS BID FROM Multivalues AS A,Multivalues AS B WHERE A.Ints.Value=B.ID ORDER BY A.ID,B.ID"
        assert run(args.cli, "query", "run", cross, *options) == grid(positive)
        controls += 1
        inner = "SELECT A.ID AS AID,B.ID AS BID FROM Multivalues AS A INNER JOIN Multivalues AS B ON A.ID<=B.ID WHERE A.Ints.Value=B.ID ORDER BY A.ID,B.ID"
        assert run(args.cli, "query", "run", inner, *options) == grid([(a,b) for a,b in positive if a<=b])
        controls += 1
        negative = [(a,b) for a,b in pairs if negate(membership(identities[a][4:], f"I32:{b}")) is True]
        assert run(args.cli, "query", "run", cross.replace("WHERE A.Ints.Value=B.ID", "WHERE NOT A.Ints.Value=B.ID"), *options) == grid(negative)
        controls += 1
        outer = [(a,a+1) for a in sorted(identities) if a+1 in identities and negate(membership(identities[a+1][4:], "I32:0")) is True]
        sql = "SELECT A.ID AS AID,B.ID AS BID FROM Multivalues AS A LEFT JOIN Multivalues AS B ON A.ID+1=B.ID WHERE NOT B.Ints.Value=0 ORDER BY A.ID"
        assert run(args.cli, "query", "run", sql, *options) == grid(outer)
        controls += 1
        outer = [(b-1 if b-1 in identities else None,b) for b in sorted(identities) if b-1 not in identities or membership(identities[b-1][4:], "I32:0") is True]
        sql = "SELECT A.ID AS AID,B.ID AS BID FROM Multivalues AS A RIGHT JOIN Multivalues AS B ON A.ID+1=B.ID WHERE A.ID IS NULL OR A.Ints.Value=0 ORDER BY B.ID"
        assert run(args.cli, "query", "run", sql, *options) == grid(outer)
        controls += 1
        derived = cross.replace("Multivalues AS A,Multivalues AS B", "(SELECT ID FROM Multivalues) AS B,Multivalues AS A")
        assert run(args.cli, "query", "run", derived, *options) == grid(positive)
        controls += 1
        expected = "AID\tN\n" + "".join(f"{a}\t{sum(x==a for x,_ in positive)}\n" for a in sorted(identities) if any(x==a for x,_ in positive))
        sql = "SELECT A.ID AS AID,Count(*) AS N FROM Multivalues AS A,Multivalues AS B WHERE A.Ints.Value=B.ID GROUP BY A.ID ORDER BY A.ID"
        assert run(args.cli, "query", "run", sql, *options) == expected
        controls += 1
        for delta in [0,1]:
            expected = grid([(a,b) for a,b in pairs if membership(identities[a][4:], f"I32:{b+delta}") is True])
            sql = "PARAMETERS Delta LONG; " + cross.replace("A.Ints.Value=B.ID", "A.Ints.Value=B.ID+Delta")
            assert run(args.cli, "query", "run", sql, *options, "--param", f"Delta=long:{delta}") == expected
            controls += 1
        text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "MultivalueOracle.java", path, "Multivalues", "Texts", "ID")
        records = [line.split("\t") for line in text.splitlines()]
        options += ["--collation", "ordinal"]
        red = "T" + "Red".encode().hex()
        expected = "ID\n" + "".join(display(row[3]) + "\n" for row in records if membership(row[4:], red) is True)
        assert run(args.cli, "query", "run", "SELECT ID FROM Multivalues WHERE Texts.Value='Red' ORDER BY ID", *options) == expected
        controls += 1
        for sql in ["SELECT ID FROM Multivalues WHERE False AND Singles.Value>0", "SELECT ID FROM Multivalues WHERE True OR Doubles.Value>0"]:
            rejected(args.cli, sql, options, "non-finite")
            controls += 1
        for sql in ["SELECT TOP 1 A.ID FROM Multivalues AS A,Multivalues AS B WHERE False AND A.Singles.Value>0", "SELECT A.ID FROM Multivalues AS A,Multivalues AS B WHERE True OR B.Doubles.Value>0"]:
            rejected(args.cli, sql, options, "non-finite")
            controls += 1
        assert digest(path) == before
        print(f"Unchanged source {before}")
    print(f"Passed {controls} parent membership model controls")


if __name__ == "__main__":
    main()
