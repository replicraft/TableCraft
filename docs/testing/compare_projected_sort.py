"""Original independent set/order CLI controls; no native Access execution."""
import argparse
from collections import Counter
import csv
from io import StringIO
from pathlib import Path
import random
import subprocess


def execute(cli, sql, expected):
    result = subprocess.run([str(cli), "query", "run", sql, "--format", "tsv"],
                            capture_output=True, check=False)
    assert result.returncode == 0, (sql, result.stderr)
    rows = list(csv.reader(StringIO(result.stdout.decode()), delimiter="\t"))[1:]
    actual = [None if row[0] == "NULL" else int(row[0]) for row in rows]
    assert actual == expected, (sql, actual, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    args = parser.parse_args()
    generator = random.Random(20261022)
    for _ in range(48):
        values = [generator.choice([None, generator.randint(-5, 5)])
                  for _ in range(generator.randint(2, 12))]
        inputs = " UNION ALL ".join(f"SELECT {'NULL' if value is None else value} AS N"
                                    for value in values)
        transformed = {None if value is None else value + 1 for value in values}
        ascending = sorted(transformed, key=lambda value: (value is not None, value or 0))
        for descending in [False, True]:
            sql = f"SELECT DISTINCT N+1 FROM ({inputs}) T ORDER BY N+1" + (" DESC" if descending else "")
            execute(args.cli, sql, list(reversed(ascending)) if descending else ascending)
        sql = f"SELECT DISTINCT Count(*) FROM ({inputs}) T GROUP BY N ORDER BY Count(*)"
        execute(args.cli, sql, sorted(set(Counter(values).values())))
    print("Passed 144 independent projected set/order CLI controls; native Access unrun.")


if __name__ == "__main__":
    main()
