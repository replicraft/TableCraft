"""Compare inert table storage/column descriptions with independent native metadata.

No linked provider is opened by the comparison or CLI. Uses the reviewed Jackcess
artifact and original MIT recipes; no native Microsoft Access qualification.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
from compare_browsing import RECIPES, PROFILES, escaped, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "BrowsingMetadataFixture.java", args.directory / f"Metadata-{profile}.{suffix}", profile)
    for profile, suffix in PROFILES:
        path = args.directory / f"Metadata-{profile}.{suffix}"
        before = path.read_bytes()
        assert before[20:24] == bytes([{"V2000": 1, "V2016": 5, "V2019": 6}[profile], 0, 0, 0])
        def oracle(recipe, *extra):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / f"{recipe}.java", path, *extra)
        records = [line.split("\t") for line in oracle("CatalogOracle").splitlines()]
        table_records = [record for record in records if int(record[2]) in (1, 4, 6)]
        details = run(args.cli, "tables", path, "--details").splitlines()
        assert details[0] == "Name\tStorage\tRows"
        names = {1: "Local", 4: "Linked ODBC", 6: "Linked non-ODBC"}
        expected = [f"{escaped(record[4])}\t{names[int(record[2])]}\t" + ("Not inspected" if int(record[2]) == 1 else "External source not opened") for record in table_records]
        assert sorted(details[1:]) == sorted(expected)
        schema = run(args.cli, "inspect", path, "--table", "Items").splitlines()
        assert "Scalar columns\tSupported carriers; rows not inspected" in schema
        assert "Unsupported columns\t0" in schema
        header = next(i for i, line in enumerate(schema) if line.startswith("Column\tNative type\t"))
        actual_columns = [line.split("\t")[:3] for line in schema[header + 1:]]
        expected_columns = [line.split("\t")[1:] for line in oracle("TableSchemaOracle", "Items").splitlines()]
        assert actual_columns == expected_columns
        for command in [["inspect", path, "--table", "Unresolved link"], ["rows", path, "Unresolved link"]]:
            result = subprocess.run(list(map(str, [args.cli, *command])), capture_output=True)
            assert result.returncode == 2 and not result.stdout
        assert path.read_bytes() == before
        print(f"{profile}: complete inventory, column metadata, unresolved-link refusals agree; unchanged SHA-256 {hashlib.sha256(before).hexdigest()}")


if __name__ == "__main__":
    main()
