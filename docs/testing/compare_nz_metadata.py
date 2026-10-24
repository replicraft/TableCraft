"""Check query Nz values and metadata-driven UNIONs over registered wide inputs.

Only TableCraft executes SQL. Inert getters and independent recipe models retain
original bytes; no Microsoft Access query qualification is claimed.
"""

import argparse
import csv
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import PROFILES, RECIPES, display
from compare_union_types import JAR_SHA256
from compare_wide_table_histories import INPUT_HASHES, NATIVE, digest, model


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    controls = 0

    for profile, suffix in PROFILES:
        path = args.directory / f"wide-{profile}.{suffix}"
        original = digest(path)
        assert original == INPUT_HASHES[profile], "Unregistered input"
        for table in ["WideVariable", "WideMixed"]:
            expected = model(table == "WideMixed")
            reference = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), table,
            ], text=True, encoding="utf-8")
            records = [line.split("\t")[1:] for line in reference.splitlines()]
            assert {int(row[0][4:]): row for row in records} == expected
            controls += 1

            def query(sql, wanted=None, options=(), fails=False):
                nonlocal controls
                result = subprocess.run([
                    str(args.cli), "query", "run", sql, "--database", str(path),
                    *NATIVE, *options,
                ], capture_output=True, text=True, encoding="utf-8")
                if fails:
                    assert result.returncode == 2 and not result.stdout, (profile, table, sql)
                else:
                    assert result.returncode == 0 and not result.stderr, (profile, table, result.stderr)
                    actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                    assert actual == wanted, (profile, table, sql, "values")
                controls += 1

            text_rows = [[str(identity), display(expected[identity][254] if expected[identity][254] != "N" else "T")]
                         for identity in sorted(expected)]
            for index in [[], ["--index", table, "IdAsc"]]:
                query(f"SELECT ID,Nz(C254) AS V FROM {table} ORDER BY ID",
                      [["ID", "V"], *text_rows], index)
                for reverse in [False, True]:
                    native = f"SELECT ID,Nz(C254) AS V FROM {table}"
                    literal = "SELECT 99 AS ID,7 AS V"
                    arms = [literal, native] if reverse else [native, literal]
                    sql = "SELECT ID,V FROM (" + " UNION ALL ".join(arms) + ") AS D ORDER BY ID"
                    query(sql, [["ID", "V"], *text_rows, ["99", display("T37")]],
                          [*index, "--number-format", "invariant"])
                    query(sql, options=index, fails=True)
                query(f"SELECT ID,Nz(ID,CInt(NULL)) FROM {table}", options=index, fails=True)

            if table == "WideMixed":
                numeric_rows = [[str(identity), display(expected[identity][127] if expected[identity][127] != "N" else "I16:0")]
                                for identity in sorted(expected)]
                query(f"SELECT ID,Nz(C127,CInt(0)) AS V FROM {table} ORDER BY ID",
                      [["ID", "V"], *numeric_rows])
                query(f"SELECT ID,CInt(C127) FROM {table}", fails=True)
        assert digest(path) == original, "Original changed"
        print(f"{profile}: unchanged SHA-256 {original}", flush=True)
    print(f"Passed {controls} Nz model/getter/query controls; native Access unrun.")


if __name__ == "__main__":
    main()
