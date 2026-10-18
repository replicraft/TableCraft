"""Compare chained-definition schemas, index metadata and query rows with Jackcess."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--ace", required=True, type=Path)
    parser.add_argument("--jet", required=True, type=Path)
    args = parser.parse_args()

    def oracle(helper, path, table):
        return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), f"docs/testing/{helper}.java",
                                        str(path), table], text=True).splitlines()

    def cli(*command):
        return subprocess.check_output([str(args.cli), *map(str, command)], text=True).splitlines()

    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        for table in ["Wide", "EmptyWide"]:
            expected_schema = [line.split("\t")[1:] for line in oracle("TableSchemaOracle", path, table)]
            schema = [line.split("\t")[:3] for line in cli("inspect", path, "--table", table)[3:]]
            if schema != expected_schema:
                raise RuntimeError(f"schema disagreement: {format} {table}")
            expected_indexes = sorted(line.split("\t")[1:] for line in oracle("IndexOracle", path, table))
            indexes = sorted(line.split("\t") for line in cli("inspect", path, "--table", table, "--indexes")[2:])
            if indexes != expected_indexes:
                raise RuntimeError(f"index metadata disagreement: {format} {table}")
            names = [column[0] for column in expected_schema]
            rows = [["NULL" if value == "N" else value.removeprefix("I32:")
                     for value in row.split("\t")[1:]] for row in oracle("TableDataOracle", path, table)]
            for index, predicate, include in [
                (None, "1=1", lambda _: True),
                ("First", f"[{names[0]}] >= 2000 AND [{names[0]}] <= 4000",
                 lambda row: 2000 <= int(row[0]) <= 4000),
                ("LastDesc", f"[{names[-1]}] IS NULL", lambda row: row[-1] == "NULL"),
            ]:
                sql = f"SELECT * FROM [{table}] WHERE {predicate} ORDER BY [{names[0]}]"
                options = [] if index is None else ["--index", table, index]
                actual = cli("query", "run", "--database", path, *options, sql)
                expected = ["\t".join(names), *["\t".join(row) for row in rows if include(row)]]
                if actual != expected:
                    raise RuntimeError(f"row disagreement: {format} {table} {index}")
                digest = hashlib.sha256("".join(row + "\n" for row in actual).encode()).hexdigest()
                print(format, table, index or "sequential", len(schema), len(actual) - 1, digest, flush=True)


if __name__ == "__main__":
    main()
