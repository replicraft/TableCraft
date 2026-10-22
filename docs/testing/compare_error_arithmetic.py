"""Check explicit arithmetic Error rejection and eager failures through the CLI."""

import argparse
import hashlib
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--directory", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.directory):
        parser.error("--jar and --directory must be supplied together")
    controls = 0

    def refused(mode, expression, diagnostic, options=(), source=""):
        nonlocal controls
        sql = "SELECT 1 AS Buffered," + expression + source
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, *map(str, options)],
            text=True, encoding="utf-8", capture_output=True,
        )
        assert result.returncode == 2 and not result.stdout, (sql, result)
        assert diagnostic in result.stderr, (sql, diagnostic, result.stderr)
        controls += 1

    rng = random.Random(20261022)
    codes = [0, 1, 2001, 32767, 65535] + [rng.randrange(65536) for _ in range(16)]
    operators = ["+", "-", "*", "/", "\\", "Mod", "^"]
    for mode in ["eval", "run"]:
        for code in codes:
            for operator in operators:
                for left, right in [
                    (f"CVErr({code})", "1"),
                    ("Null", f"CVErr({code})"),
                    (f"CVErr({code})", "CVErr(0)"),
                ]:
                    refused(mode, f"({left}) {operator} ({right})", "expression type mismatch")
            for sign in ["+", "-"]:
                refused(mode, f"{sign}CVErr({code})", "expression type mismatch")
        for operator in operators:
            for left, right, diagnostic in [
                ("CVErr(0)", "1/0", "division by zero"),
                ("1/0", "CVErr(0)", "division by zero"),
                ("CVErr(0)", "CVErr(-1)", "invalid arithmetic domain"),
                ("CVErr(-1)", "1/0", "invalid arithmetic domain"),
            ]:
                refused(mode, f"({left}) {operator} ({right})", diagnostic)

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        for suffix, digest in INPUT_HASHES.items():
            path = args.directory / ("index-" + suffix)
            before = path.read_bytes()
            assert hashlib.sha256(before).hexdigest() == digest
            output = subprocess.check_output(
                ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                 str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"],
                text=True, encoding="utf-8",
            )
            tags = [line.split("\t")[1] for line in output.splitlines()]
            assert len(tags) == 20000 and all(tag.startswith("I32:") for tag in tags)
            ids = [int(tag.removeprefix("I32:")) for tag in tags]
            assert all(0 <= value <= 65535 for value in ids)
            budgets = [
                "--database", path,
                "--native-bytes", "67108864",
                "--native-rows", "100000",
                "--native-steps", "10000000",
                "--execution-steps", "10000000",
            ]
            for index in [None, "IdAsc", "IdDesc"]:
                options = budgets if index is None else [*budgets, "--index", "IndexedRows", index]
                for operator in operators:
                    for right, diagnostic in [
                        ("1", "expression type mismatch"),
                        ("1/(ID-ID)", "division by zero"),
                    ]:
                        refused(
                            "run", f"CVErr(ID) {operator} ({right})", diagnostic,
                            options, " FROM IndexedRows WHERE ID>=10 AND ID<=20",
                        )
            assert path.read_bytes() == before
    print(f"Passed {controls} independent arithmetic Error/eager/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
