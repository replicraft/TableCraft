"""Compare automatic native selections with independent inert stored getters.

Only TableCraft executes SQL. Existing registered inputs remain unchanged.
"""

import argparse
import csv
from decimal import Decimal
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_large_indexes import FORMATS, INPUT_HASHES, JAR_SHA256, registered_input_hash
from compare_large_tables import digest

RECIPES = Path(__file__).resolve().parent


def query(cli, path, sql, options=(), accepted=True):
    result = subprocess.run(
        [str(cli), "query", "run", sql, "--database", str(path), "--auto-index", *options],
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=False,
    )
    assert result.returncode == (0 if accepted else 2), (path, sql, result.stderr)
    if not accepted:
        assert not result.stdout, (path, sql, result.stdout[:100])
        return None
    assert not result.stderr, result.stderr
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    controls = 0
    larger = ["--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000"]
    larger += ["--execution-steps", "10000000"]

    with tempfile.TemporaryDirectory(prefix="tablecraft-auto-index-getters-") as temporary:
        subprocess.run(
            [
                "javac", "-cp", str(args.jar), "-d", temporary,
                str(RECIPES / "IndexRowsOracle.java"), str(RECIPES / "TableDataOracle.java"),
            ],
            check=True,
        )
        java = ["java", "-Dstdout.encoding=UTF-8", "-cp", os.pathsep.join((str(args.jar), temporary))]
        for suffix, profile in FORMATS.items():
            path = args.directory / ("index-" + suffix)
            original = registered_input_hash(path, INPUT_HASHES[suffix])
            columns = ["ID", "Bucket", "ByteValue", "ShortValue", "Money", "GuidValue"]
            if profile in ("V2016", "V2019"):
                columns.append("BigValue")
            raw = subprocess.check_output(
                [*java, "IndexRowsOracle", str(path), "IndexedRows", "IdAsc", *columns],
                text=True,
                encoding="utf-8",
            )
            rows = []
            for line in raw.splitlines():
                values = line.split("\t")
                row = dict(zip(columns, values, strict=True))
                for column in ["ID", "Bucket", "ByteValue", "ShortValue", *columns[6:]]:
                    row[column] = None if row[column] == "NULL" else int(row[column])
                row["Money"] = None if row["Money"] == "NULL" else Decimal(row["Money"])
                row["GuidValue"] = None if row["GuidValue"] == "NULL" else row["GuidValue"].strip("{}").lower()
                rows.append(row)
            assert [row["ID"] for row in rows] == list(range(1, 20001))
            sequential = subprocess.check_output(
                [*java, "TableDataOracle", str(path), "IndexedRows"],
                text=True,
                encoding="utf-8",
            )
            physical_ids = [
                int(line.split("\t", 2)[1].removeprefix("I32:"))
                for line in sequential.splitlines()
            ]
            assert sorted(physical_ids) == list(range(1, 20001))
            guid = "6b29fc40-ca47-1067-b31d-00dd010662da"
            cases = [
                ("ID=19999", lambda row: row["ID"] == 19999),
                ("ID>=19997", lambda row: row["ID"] >= 19997),
                ("ID<0", lambda row: row["ID"] < 0),
                ("Bucket=1", lambda row: row["Bucket"] == 1),
                ("Bucket IS NULL", lambda row: row["Bucket"] is None),
                ("ByteValue=127", lambda row: row["ByteValue"] == 127),
                ("ByteValue IS NULL", lambda row: row["ByteValue"] is None),
                ("ShortValue<0", lambda row: row["ShortValue"] is not None and row["ShortValue"] < 0),
                ("ShortValue IS NULL", lambda row: row["ShortValue"] is None),
                ("[Money]=CCur(0.0001)", lambda row: row["Money"] == Decimal("0.0001")),
                ("[Money] IS NULL", lambda row: row["Money"] is None),
                ("GuidValue={GUID {" + guid + "}}", lambda row: row["GuidValue"] == guid),
                ("GuidValue IS NULL", lambda row: row["GuidValue"] is None),
            ]
            if "BigValue" in columns:
                cases.append((
                    "BigValue>=9007199254740995",
                    lambda row: row["BigValue"] is not None and row["BigValue"] >= 9007199254740995,
                ))

            for predicate, matches in cases:
                expected = [["ID"], *[[str(row["ID"])] for row in rows if matches(row)]]
                sql = "SELECT ID FROM IndexedRows WHERE " + predicate + " ORDER BY ID"
                # A full 20,000-row scan exceeds defaults. These successful
                # default-limit reads therefore also establish actual selection.
                assert query(args.cli, path, sql) == expected, (path, predicate)
                controls += 1

            lookup = {row["ID"]: row for row in rows}
            physical_matches = [
                identifier for identifier in physical_ids
                if lookup[identifier]["ShortValue"] is not None and lookup[identifier]["ShortValue"] >= 0
            ]
            sql = "SELECT ID FROM IndexedRows WHERE ShortValue>=0"
            expected = [["ID"], *[[str(identifier)] for identifier in physical_matches]]
            assert query(args.cli, path, sql) == expected
            sql = "SELECT TOP 3 ID FROM IndexedRows WHERE ShortValue>=0"
            expected = [["ID"], *[[str(identifier)] for identifier in physical_matches[:3]]]
            assert query(args.cli, path, sql) == expected
            sql = "SELECT First(ID) AS FirstID,Last(ID) AS LastID FROM IndexedRows WHERE ShortValue>=0"
            expected = [
                ["FirstID", "LastID"],
                [str(physical_matches[0]), str(physical_matches[-1])],
            ]
            assert query(args.cli, path, sql) == expected
            controls += 3

            sql = "PARAMETERS P LONG; SELECT ID FROM IndexedRows WHERE ID=P"
            assert query(args.cli, path, sql, ["--param", "P=long:19999"]) == [["ID"], ["19999"]]
            controls += 1
            sql = "SELECT (SELECT Count(*) FROM IndexedRows) AS Total,ID FROM IndexedRows WHERE ID=20000"
            assert query(args.cli, path, sql, larger) == [["Total", "ID"], ["20000", "20000"]]
            controls += 1
            for sql, options in [
                ("SELECT ID FROM IndexedRows", []),
                ("SELECT ID FROM IndexedRows WHERE ID=20000 AND 1/0=0", larger),
                ("SELECT ID FROM IndexedRows WHERE [Money]=922337203685478", larger),
            ]:
                query(args.cli, path, sql, options, accepted=False)
                controls += 1
            assert digest(path) == original
            print(path.name, original, "passed", flush=True)

    print(f"Passed {controls} independent automatic-index controls; native Access unrun.")


if __name__ == "__main__":
    main()
