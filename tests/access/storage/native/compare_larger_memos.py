"""Original larger Memo/getter controls; only TableCraft executes SQL."""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest


RECIPES = Path(__file__).resolve().parent
PROFILES = {"jet4": "V2000", "2007": "V2007", "2016": "V2016", "2019": "V2019"}
INPUT_HASHES = {
    "jet4": "9c178ae4ebbaaf2e3965b7c011c6dab9f8f2ed2eb8331c492ae994d00592eb54",
    "2007": "f03358973b9f15f9de0451a314a437522e00fe0ab6d57bd4773502c025473580",
    "2016": "90cfc95517dab8bf5da482b59dcc1443ab8a323d5bf70d532b1c6fecb2aa74d4",
    "2019": "c0ddab22165559475d04d2298ad768d4a001edb0959aa0c93a9f5b5fcbfaa9b9",
}
POOLS = [
    "--native-bytes", "268435456", "--native-steps", "100000000",
    "--execution-bytes", "268435456", "--execution-steps", "100000000",
    "--output-bytes", "16777216", "--format", "tsv",
]


def changed(options, flag, value):
    result = list(options)
    result[result.index(flag) + 1] = value
    return result


def java(jar, *arguments):
    return subprocess.check_output(
        ["java", "-Dstdout.encoding=UTF-8", "-cp", str(jar),
         str(RECIPES / "LargerMemoFixture.java"), *map(str, arguments)],
        text=True, encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--report", type=Path, help="Fresh JSON evidence path; existing files are refused")
    args = parser.parse_args()
    if args.report is not None and args.report.exists():
        parser.error("--report must not exist")
    binary_hash = digest(args.cli)
    assert digest(args.jar) == JAR_SHA256
    if args.generate:
        args.directory.mkdir(parents=True, exist_ok=False)

    csv.field_size_limit(16 * 1024 * 1024)
    controls = 0
    inventory = {}
    expected = ["a" * 3_145_728, "中" * 600_000, "é🦀" * 300_000, "", None]
    large = [*POOLS, "--native-value-bytes", "16777216", "--native-text-bytes", "16777216",
             "--text-bytes", "16777216"]

    def query(path, sql, options, accepted=True):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, "--database", str(path), *options],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        controls += 1
        if not accepted:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert "limit" in result.stderr.lower(), result.stderr
            return None
        assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
        return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))

    for suffix, profile in PROFILES.items():
        path = args.directory / ("memos-" + suffix + (".mdb" if suffix == "jet4" else ".accdb"))
        if args.generate:
            java(args.jar, path, profile)
        original = digest(path)
        if not args.generate:
            assert original == INPUT_HASHES[suffix], (suffix, original)
        oracle = java(args.jar, "--inspect", path).splitlines()
        wanted = []
        for identity, value in enumerate(expected, 1):
            data = None if value is None else value.encode("utf-8")
            units = -1 if value is None else len(value.encode("utf-16-le")) // 2
            wanted.append(f"{identity}\t{units}\t{len(data) if data is not None else -1}\t"
                          + (hashlib.sha256(data).hexdigest() if data is not None else "NULL"))
        assert oracle == wanted, (suffix, oracle, wanted)

        assert query(path, "SELECT Count(*) AS N FROM LargeMemos", POOLS) == [["N"], ["5"]]
        rows = query(path, "SELECT ID,Len([Memo]) AS N FROM LargeMemos ORDER BY ID", large)
        assert rows == [["ID", "N"], ["1", "3145728"], ["2", "600000"],
                        ["3", "900000"], ["4", "0"], ["5", "NULL"]]
        for flag, limit in [("--native-value-bytes", "2097152"),
                            ("--native-text-bytes", "1048576"), ("--text-bytes", "1048576"),
                            ("--native-bytes", "1048576"), ("--native-steps", "1000"),
                            ("--execution-bytes", "1048576"), ("--execution-steps", "1000")]:
            query(path, "SELECT 1,Len([Memo]) FROM LargeMemos", changed(large, flag, limit), False)

        for identity, value in enumerate(expected, 1):
            rows = query(path, f"SELECT [Memo] FROM LargeMemos WHERE ID={identity}",
                         [*large, "--auto-index"])
            actual = rows[1][0]
            if value is None:
                assert actual == "NULL", (identity, actual)
            else:
                assert json.loads(actual) == value, identity

        selected = [*large, "--auto-index"]
        selected = changed(selected, "--native-value-bytes", "2097152")
        selected = changed(selected, "--native-text-bytes", "1800000")
        selected = changed(selected, "--text-bytes", "1800000")
        assert query(path, "SELECT Len([Memo]) AS N FROM LargeMemos WHERE ID=2", selected) == [["N"], ["600000"]]
        query(path, "SELECT [Memo] FROM LargeMemos WHERE ID=2",
              changed(selected, "--native-text-bytes", "1799999"), False)
        query(path, "SELECT [Memo] FROM LargeMemos WHERE ID=2",
              changed(selected, "--text-bytes", "1799999"), False)
        query(path, "SELECT 1,[Memo] FROM LargeMemos WHERE ID=1",
              changed(large, "--output-bytes", "1048576"), False)
        assert query(path, "SELECT Count(*) AS N FROM LargeMemos", POOLS) == [["N"], ["5"]]
        assert digest(path) == original
        inventory[suffix] = {"bytes": path.stat().st_size, "sha256": original}
        print(suffix, inventory[suffix], flush=True)

    assert digest(args.cli) == binary_hash
    report = {
        "controls": controls,
        "inputs": inventory,
        "native_access": "unrun",
        "scope": "larger decoded Memo query values, exact getter/bytes/UTF16-unit answers and bounded failures",
        "binary_sha256": binary_hash,
        "runner_sha256": digest(Path(__file__)),
        "recipe_sha256": digest(RECIPES / "LargerMemoFixture.java"),
        "jar_sha256": JAR_SHA256,
    }
    if args.report is not None:
        with args.report.open("x", encoding="utf-8") as output:
            json.dump(report, output, indent=2, sort_keys=True)
            output.write("\n")
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
