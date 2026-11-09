"""Compare Text/Binary NULL index selections with inert sequential getters."""

import argparse
import csv
from io import StringIO
import json
from pathlib import Path
import subprocess
import tempfile

from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest

RECIPES = Path(__file__).resolve().parent
PROFILES = {"V2000": "mdb", "V2007": "accdb", "V2016": "accdb", "V2019": "accdb"}
INPUT_HASHES = {
    "Binary-V2000": "fbe38ca0eae7f201209a1624aca2963f0709eed6b1af72dbb66bc8a0abec513c",
    "Binary-V2007": "51024e0994d9f62133d0eb99b6648933cbcfbe07224fad14ac14ea5c62542bbd",
    "Binary-V2016": "1f82f80b8c36a824fdc407da51785a6cfc2e4bfd5bd9cfcbdcdbfa9831d04c14",
    "Binary-V2019": "6cc2a2b534a133d96d354307dc22431d697edf133a9027a69728215e87faa712",
    "Text-V2000-0": "5f989e1f46f953680c4a46e6fe3ae2247c5f481a3f519bbb4d2f7d4ae283492e",
    "Text-V2000-1": "d0ef874403b899acc8e9cc459ea8def96829c29ed2a84b65af57f5665355d2a5",
    "Text-V2007-0": "4e0c201a212c33a885e7388e5f59e1d98d22d44018f144d0405f77e44665258c",
    "Text-V2007-1": "145a7218d837ffcc43d6c2ab9fef961dafa3d2d3cdc3cc1efc694c38d19a2fe3",
    "Text-V2016-0": "b80e74b69f36fdd39b53f9b1819941d5c0d237a904ae1d63c091289c57f4e775",
    "Text-V2016-1": "7da85ab409ec86076be327c275afd72fc46b826b2e937887226927139a562826",
    "Text-V2019-0": "776d826dd5c2f0c064155ce932ce0dd7a5e9738a17f2a49b774cdd48f280533d",
    "Text-V2019-1": "d23a10dea54fe2e20e210a4886452ac8db3222f3aa4af98e558dcd7c993328c2",
}
POOLS = ["--native-bytes", "268435456", "--native-steps", "100000000", "--format", "tsv"]


def java(jar, name, *arguments):
    return subprocess.check_output(
        ["java", "-Dstdout.encoding=UTF-8", "-cp", str(jar), str(RECIPES / name), *map(str, arguments)],
        text=True, encoding="utf-8",
    )


def query(cli, path, sql, options=(), failure=None):
    result = subprocess.run(
        [str(cli), "query", "run", sql, "--database", str(path), *POOLS, *options],
        capture_output=True, text=True, encoding="utf-8", check=False,
    )
    if failure is not None:
        assert result.returncode == 2 and not result.stdout, (sql, result.stderr)
        assert failure in result.stderr.lower(), result.stderr
        return None
    assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    if args.generate:
        args.directory.mkdir(parents=True, exist_ok=False)
        for profile, extension in PROFILES.items():
            for kind, version in [("Text", 0), ("Text", 1), ("Binary", None)]:
                label = f"{kind}-{profile}" + (f"-{version}" if version is not None else "")
                path = args.directory / f"{label}.{extension}"
                parameters = [path, profile] + ([version] if version is not None else [])
                java(args.jar, kind + "IndexFixture.java", *parameters)
    controls = 0
    inventory = {}
    for profile, extension in PROFILES.items():
        for kind, version in [("Text", 0), ("Text", 1), ("Binary", None)]:
            label = f"{kind}-{profile}" + (f"-{version}" if version is not None else "")
            path = args.directory / f"{label}.{extension}"
            original = digest(path)
            if not args.generate:
                assert original == INPUT_HASHES[label], (label, original)
            table = kind + "Indexed"
            column = "Label" if kind == "Text" else "Bytes"
            rows = [line.split("\t") for line in java(
                args.jar, "TableDataOracle.java", path, table).splitlines()]
            assert len(rows) == 1000
            nulls = [int(row[1].removeprefix("I32:")) for row in rows if row[2] == "N"]
            empty = "T" if kind == "Text" else "X"
            assert nulls and any(row[2] == empty for row in rows)
            expected = [["ID"], *[[str(value)] for value in nulls]]
            sql = f"SELECT ID FROM [{table}] WHERE [{column}] IS NULL"
            assert query(args.cli, path, sql) == expected
            automatic = ["--auto-index", "--native-rows", "512"]
            assert query(args.cli, path, sql, automatic) == expected
            query(args.cli, path, sql, ["--native-rows", "512"], "limit")
            for index in [kind + "Asc", kind + "Desc"]:
                assert query(args.cli, path, sql, ["--index", table, index, "--native-rows", "512"]) == expected
            assert query(args.cli, path, f"SELECT TOP 3 ID FROM [{table}] WHERE [{column}] IS NULL", automatic) == expected[:4]
            first_last = f"SELECT First(ID) AS F,Last(ID) AS L FROM [{table}] WHERE [{column}] IS NULL"
            assert query(args.cli, path, first_last, automatic) == [["F", "L"], [str(nulls[0]), str(nulls[-1])]]
            query(args.cli, path, sql + f" UNION ALL SELECT Count(*) FROM [{table}]", automatic, "limit")
            assert query(args.cli, path, sql + f" UNION ALL SELECT Count(*) FROM [{table}]", ["--auto-index"]) == [*expected, ["1000"]]
            query(args.cli, path, sql.replace(" IS NULL", " IS NOT NULL"), automatic, "limit")
            query(args.cli, path, sql + " AND 1/0=0", ["--auto-index"], "division")
            query(args.cli, path, sql, ["--index", table, "PrimaryKey", "--native-rows", "512"], "limit")
            assert query(args.cli, path, f"SELECT ID FROM [{kind}Empty] WHERE [{column}] IS NULL", automatic) == [["ID"]]
            assert query(args.cli, path, f"SELECT ID FROM [{kind}Ignored] WHERE [{column}] IS NULL", automatic) == [["ID"]]
            query(args.cli, path, f"SELECT ID FROM [{kind}Ignored] WHERE [{column}] IS NULL", ["--index", kind + "Ignored", "Ignored" + kind], "not implemented")
            metadata = [line.split("\t") for line in java(args.jar, "IndexOracle.java", path, table).splitlines()]
            root = next(int(row[4]) for row in metadata if row[1] == kind + "Asc")
            damaged = bytearray(path.read_bytes())
            damaged[root * 4096] = 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-null-index-") as temporary:
                candidate = Path(temporary) / path.name
                candidate.write_bytes(damaged)
                query(args.cli, candidate, sql, ["--auto-index"], "source")
            assert digest(path) == original
            inventory[label] = {"bytes": path.stat().st_size, "sha256": original}
            controls += 18
            print(label, "passed", flush=True)
    print(json.dumps({"controls": controls, "inputs": inventory}, indent=2))


if __name__ == "__main__":
    main()
