"""Compare the registered saved wildcard join with independent inert row getters.

Only TableCraft executes SQL. This is not native Access query qualification.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess

from compare_browsing import display, run

DATABASE_SHA256 = "3cd1fc88d70bc93909ae6d190fa9692e709d5641b205fd2f66185c905e42cfbf"
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
RECIPES = Path(__file__).resolve().parent
TABLES = ["TAG_NME_TB", "TAG_GRP_TB"]
EXPECTED_ROWS = 933


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--database", type=Path, default=RECIPES.parent / "examples/Access_Example_VBA.accdb")
    args = parser.parse_args()
    before = hashlib.sha256(args.database.read_bytes()).hexdigest()
    assert before == DATABASE_SHA256, "Registered input hash mismatch"
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256, "Reviewed JAR hash mismatch"

    def oracle(recipe):
        return subprocess.check_output(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), str(RECIPES / (recipe + ".java")), str(args.database), *TABLES],
            text=True, encoding="utf-8",
        ).splitlines()

    columns = {name: [] for name in TABLES}
    for line in oracle("TableSchemaOracle"):
        table, column, *_ = line.split("\t")
        columns[table].append(column)
    rows = {name: [] for name in TABLES}
    for line in oracle("TableDataOracle"):
        table, *values = line.split("\t")
        rows[table].append(values)

    left_key = columns[TABLES[0]].index("TAG_GRP_ID")
    right_key = columns[TABLES[1]].index("TAG_GRP_ID")
    right_label = columns[TABLES[1]].index("TAG_GRP_NME")
    joined = [left + [right[right_label]]
              for left in rows[TABLES[0]] for right in rows[TABLES[1]]
              if left[left_key] != "N" and left[left_key] == right[right_key]]
    assert len(joined) == EXPECTED_ROWS, "Incomplete independent join inputs"
    expected = "\t".join(columns[TABLES[0]] + ["TAG_GRP_NME"]) + "\n"
    expected += "".join("\t".join(display(value) for value in row) + "\n" for row in joined)
    actual = run(args.cli.resolve(), "query", "run", "--saved", "950_Leszynski_Conventions_qry", "--database", args.database)
    assert actual == expected, "Saved wildcard join differs from independent rows"
    assert hashlib.sha256(args.database.read_bytes()).hexdigest() == before, "Original changed"
    print(f"{EXPECTED_ROWS} complete saved-join rows agree; original unchanged; native Access SQL remains unqualified")


if __name__ == "__main__":
    main()
