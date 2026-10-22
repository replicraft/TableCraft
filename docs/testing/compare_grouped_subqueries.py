"""Original partition-model/CLI controls for grouped correlations, not native Access."""
import argparse
from collections import defaultdict
import csv
import io
from pathlib import Path
import random
import subprocess


def execute(cli, rows, filter_groups):
    inputs = " UNION ALL ".join(
        f"SELECT {bucket} AS Bucket,{'NULL' if amount is None else amount} AS Amount"
        for bucket, amount in rows
    )
    having = (
        f" HAVING EXISTS(SELECT 1 FROM ({inputs}) I WHERE I.Bucket=O.Bucket AND I.Amount>0)"
        if filter_groups else ""
    )
    sql = (
        "SELECT O.Bucket,Count(*),"
        f"(SELECT Sum(I.Amount) FROM ({inputs}) I WHERE I.Bucket=O.Bucket) "
        f"FROM ({inputs}) O GROUP BY O.Bucket{having} ORDER BY O.Bucket"
    )
    result = subprocess.run([str(cli), "query", "run", sql, "--format", "tsv"],
                            capture_output=True, check=False)
    assert result.returncode == 0, (sql, result.stderr)
    actual = list(csv.reader(io.StringIO(result.stdout.decode()), delimiter="\t"))[1:]
    groups = defaultdict(list)
    for bucket, amount in rows:
        groups[bucket].append(amount)
    expected = []
    for bucket, amounts in sorted(groups.items()):
        values = [amount for amount in amounts if amount is not None]
        if filter_groups and not any(value > 0 for value in values):
            continue
        expected.append([str(bucket), str(len(amounts)), str(sum(values)) if values else "NULL"])
    assert actual == expected, (rows, filter_groups, actual, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    generator = random.Random(20261022)
    cases = [[(1, None), (1, None), (2, 3)], [(1, 2), (1, 2), (-1, -3)]]
    for _ in range(46):
        cases.append([(generator.randint(-2, 2), generator.choice([None, generator.randint(-100, 100)]))
                      for _ in range(generator.randint(2, 12))])
    for rows in cases:
        for filtered in [False, True]:
            execute(args.cli, rows, filtered)
    bad = "SELECT (SELECT O.Amount) FROM (SELECT 1 AS Bucket,2 AS Amount) O GROUP BY O.Bucket"
    result = subprocess.run([str(args.cli), "query", "run", bad, "--format", "tsv"],
                            capture_output=True, check=False)
    assert result.returncode != 0 and not result.stdout, result
    print(f"Passed {len(cases) * 2} independent grouped-correlation CLI controls and buffered refusal; native Access unrun.")


if __name__ == "__main__":
    main()
