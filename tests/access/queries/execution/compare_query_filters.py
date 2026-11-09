"""Compare CLI Int32 predicates with independent filtering of Jackcess cursor rows."""

from cli_tsv import display_tsv
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
    for family in ("2007", "2016", "2019"):
        parser.add_argument("--ace" + family, type=Path)
    args = parser.parse_args()
    controls = 0
    cases = [
        ("zero", "{key} = 0", lambda v: v == 0, []),
        ("closed", "-3 <= {key} AND {key} <= 4", lambda v: -3 <= v <= 4, []),
        ("open", "-1 < {key} AND {key} < 3", lambda v: -1 < v < 3, []),
        ("tail", "{key} >= 495", lambda v: v >= 495, []),
        ("null", "{key} IS NULL", None, []),
        ("numeric", "{key} IS NOT NULL", lambda _: True, []),
        ("empty", "{key} > 2147483647", lambda _: False, []),
        ("parameter", "{key} = p", lambda v: v == 0, ["--param", "p=long:0"]),
    ]
    profiles = [("ace", args.ace), ("jet4", args.jet)] + [
        ("ace" + family, getattr(args, "ace" + family))
        for family in ("2007", "2016", "2019") if getattr(args, "ace" + family)
    ]
    for format, path in profiles:
        before = path.read_bytes()
        for index, column, key in [("PrimaryKey", 0, "ID"), ("CategoryDesc", 1, "Category")]:
            reference = subprocess.check_output([
                "java", "-cp", str(args.jar), "docs/testing/IndexRowsOracle.java", str(path),
                "Indexed", index, "ID", "Category",
            ], text=True).splitlines()
            for name, predicate, include, parameters in cases:
                expected = []
                for row in reference:
                    value = row.split("\t")[column]
                    matches = (value == "NULL") if include is None else (
                        value != "NULL" and include(int(value))
                    )
                    if matches:
                        expected.append(row)
                expected.sort(key=lambda row: int(row.split("\t")[0]))
                sql = "SELECT ID, Category FROM Indexed WHERE " + predicate.format(key=key) + " ORDER BY ID"
                if parameters:
                    sql = "PARAMETERS p LONG; " + sql
                output = display_tsv(subprocess.check_output([
                    str(args.cli), "query", "run", "--database", str(path), "--index", "Indexed", index,
                    sql, *parameters,
                ], text=True)).splitlines()
                if output != ["ID\tCategory", *expected]:
                    raise RuntimeError(f"query disagreement: {format} {index} {name}")
                digest = hashlib.sha256("".join(row + "\n" for row in output).encode()).hexdigest()
                print(format, index, name, len(expected), digest, flush=True)
                controls += 1
                # Keep ORDER BY/filter inputs when they are absent from output.
                for fields, selected in [
                    ("ID", [row.split("\t")[0] for row in expected]),
                    ("Category", [row.split("\t")[1] for row in expected]),
                    ("1 AS N", ["1" for _ in expected]),
                    ("Count(*) AS N", [str(len(expected))]),
                ]:
                    sql = "SELECT " + fields + " FROM Indexed WHERE " + predicate.format(key=key)
                    if not fields.startswith("Count"):
                        sql += " ORDER BY ID"
                    if parameters:
                        sql = "PARAMETERS p LONG; " + sql
                    header = fields if " AS " not in fields else "N"
                    for selection in (["--index", "Indexed", index], []):
                        output = display_tsv(subprocess.check_output([
                            str(args.cli), "query", "run", "--database", str(path),
                            sql, *selection, *parameters,
                        ], text=True)).splitlines()
                        if output != [header, *selected]:
                            raise RuntimeError(f"projected query disagreement: {format} {index} {name} {fields} {selection}")
                        controls += 1
        assert path.read_bytes() == before, "Read command changed original bytes"
    print(controls, "indexed/sequential predicate/projection controls passed; native Access SQL unrun")


if __name__ == "__main__":
    main()
