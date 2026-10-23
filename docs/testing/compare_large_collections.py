"""Independent large parent/child models; no native Access SQL qualification."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2016": "3024aeb7ffd25831245d8ab3407bf03c80ad1fd367d6bd35ae09cb60415fd459",
    "V2007": "a298effd91afab7c51d00781fc8cfc93ddbf69c72ee8d65ba954e24c78080938",
}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    recipe = RECIPES / "LargeCollectionFixture.java"
    profiles = ["V2016", "V2007"]
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile in profiles:
            subprocess.run([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), str(recipe),
                str(args.directory / f"collections-{profile}.accdb"), profile,
            ], check=True)

    child_values = [None if value % 17 == 0 else value % 7 for value in range(100)]
    models = {
        "ManyParents": {identity: [] if identity % 5 == 0 else [identity, None, identity]
                        for identity in range(1, 20_001)},
        "ManyChildren": {identity: child_values for identity in range(1, 401)},
        "Overflow": {1: list(range(101))},
    }
    controls = 0
    native = ["--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "100000000"]
    bounded_native = ["--native-bytes", "268435456", "--native-rows", "16384", "--native-steps", "100000000"]
    execution = ["--execution-bytes", "268435456", "--execution-rows", "1000000", "--execution-steps", "100000000"]
    for profile in profiles:
        path = args.directory / f"collections-{profile}.accdb"
        before = digest(path)
        if not args.generate:
            assert before == INPUT_HASHES[profile], (profile, "unregistered fixture")
        with path.open("rb") as source:
            prefix = source.read(24)
        assert prefix[20:24] == bytes([5 if profile == "V2016" else 2, 0, 0, 0])
        stored = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), str(recipe), "--inspect", str(path),
        ], text=True, encoding="utf-8")
        observed = {name: {} for name in models}
        for line in stored.splitlines():
            name, identity, *values = line.split("\t")
            identity = int(identity)
            assert identity not in observed[name]
            observed[name][identity] = [None if value == "N" else int(value) for value in values]
        assert observed == models

        def query(sql, expected=None, options=(), diagnostic=None):
            nonlocal controls
            result = subprocess.run([
                str(args.cli), "query", "run", sql, "--database", str(path), *options,
            ], text=True, encoding="utf-8", capture_output=True)
            if diagnostic:
                assert result.returncode == 2 and not result.stdout, (profile, sql, result)
                assert diagnostic in result.stderr, (profile, sql, result.stderr)
            else:
                assert result.returncode == 0 and not result.stderr, (profile, sql, result.stderr)
                rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert rows[1:] == expected, (profile, sql, rows, expected)
            controls += 1

        for table in ["ManyParents", "ManyChildren"]:
            model = models[table]
            flattened = [value for values in model.values() for value in (values or [None])]
            values = [value for value in flattened if value is not None]
            expected = [[str(len(flattened)), str(len(values)), str(sum(values))]]
            sql = f"SELECT Count(*),Count(Numbers.Value),Sum(Numbers.Value) FROM {table}"
            for flag in ["--multivalues", "--collections"]:
                query(sql, expected, [flag, *native, *execution])
            query(sql, options=["--multivalues", *execution], diagnostic="limit")
            query(sql, options=["--multivalues", *bounded_native, *execution], diagnostic="limit")

        query("SELECT ID,Numbers.Value AS V FROM ManyParents WHERE ID IN(1,5,20000) ORDER BY ID,V",
              [["1", "NULL"], ["1", "1"], ["1", "1"], ["5", "NULL"], ["20000", "NULL"]],
              ["--multivalues", *native, *execution])
        query("SELECT Count(Numbers.Value) FROM Overflow",
              options=["--multivalues", *native, *execution], diagnostic="limit")
        query("SELECT Count(*) FROM Overflow", [["1"]], [*native, *execution])
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: 20,000/400 parents, 48,000/40,000 children; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} large collection controls; native Access unrun.")


if __name__ == "__main__":
    main()
