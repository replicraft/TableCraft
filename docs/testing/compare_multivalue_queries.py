"""Compare explicit native .Value queries with independent stored collection rows.

Python expands original parent/member values read through reviewed Jackcess APIs.
This is not Microsoft Access SQL, optimizer, locale or return-promotion evidence.
"""
import argparse
import subprocess
from pathlib import Path

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest


def rejected(cli, sql, options, reason=None):
    result = subprocess.run([str(cli), "query", "run", sql, *map(str, options)], capture_output=True, text=True)
    assert result.returncode != 0 and result.stdout == "", sql
    if reason:
        assert reason in result.stderr, (sql, result.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2007", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    controls = 0
    for path in [args.ace2007, args.ace2016, args.ace2019]:
        before = digest(path)
        options = ["--database", path, "--multivalues", "--native-bytes", "67108864"]
        collections = {}
        for column in ["Bytes", "Shorts", "Ints", "Guids", "Decimals", "Texts"]:
            text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "MultivalueOracle.java", path, "Multivalues", column, "ID")
            records = [line.split("\t") for line in text.splitlines()]
            collections[column] = records
            flat = [(display(row[3]), display(value)) for row in records for value in (row[4:] or ["N"])]
            expected = "ID\tV\n" + "".join(f"{parent}\t{value}\n" for parent, value in flat)
            sql = f"SELECT ID,[{column}].Value AS V FROM Multivalues ORDER BY ID"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
            expected = "ID\tTotal\tPresent\n" + "".join(f"{display(row[3])}\t{max(1,len(row[4:]))}\t{sum(value != 'N' for value in row[4:])}\n" for row in records)
            sql = f"SELECT ID,Count(*) AS Total,Count([{column}].Value) AS Present FROM Multivalues GROUP BY ID ORDER BY ID"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
            expected = "ID\tV\n" + "".join(f"{display(row[3])}\t{display(value)}\n" for row in records for value in row[4:] if value != "N")
            sql = f"SELECT ID,[{column}].Value AS V FROM Multivalues WHERE [{column}].Value IS NOT NULL ORDER BY ID"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
        left, right = collections["Bytes"], collections["Shorts"]
        expected = "ID\tA\tB\n" + "".join(f"{display(a[3])}\t{display(x)}\t{display(y)}\n" for a, b in zip(left,right) for x in (a[4:] or ["N"]) for y in (b[4:] or ["N"]))
        sql = "SELECT ID,Bytes.Value AS A,Shorts.Value AS B FROM Multivalues ORDER BY ID"
        assert run(args.cli, "query", "run", sql, *options) == expected, path
        controls += 1
        assert run(args.cli, "query", "run", "SELECT Count(*) AS Total FROM Multivalues", *options) == "Total\n4\n"
        controls += 1
        for sql in ["SELECT Singles.Value FROM Multivalues", "SELECT Doubles.Value FROM Multivalues", "SELECT Items.Value FROM Overflow", "SELECT ID FROM Multivalues WHERE Ints.Value=1"]:
            rejected(args.cli, sql, options, "non-finite" if "Singles.Value" in sql or "Doubles.Value" in sql else None)
            controls += 1
        rejected(args.cli, "SELECT Ints.Value FROM Multivalues", ["--database", path])
        controls += 1
        assert digest(path) == before
        print(f"Unchanged source {before}")
    print(f"Passed {controls} stored-row/model controls")


if __name__ == "__main__":
    main()
