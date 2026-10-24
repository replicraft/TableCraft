"""Check original 255-column histories using inert getters and Python values.

Only TableCraft executes SQL. This is independent read evidence, not native
Access DDL/save/reopen qualification. All database inputs stay unchanged.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_browsing import PROFILES, RECIPES, display
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2000": "64cddcafaca73413704ffcd11b97d9ca070b104515c1532ed1094c65d0080b00",
    "V2007": "80cb59292e120646ac813aff2732e81b09e747fe6546d0e6490bf4bd2ff64583",
    "V2016": "ea5bf13020f403c07d890f41d4d69cbc819add5797cb80644c1e84e37b242775",
    "V2019": "b560cb9a3c9a6ad4648393f4ab06763f821830b952529b7201e63000b6aec3bc",
}
WIDTHS = [8, 9, 16, 17, 127, 128, 129, 254, 255]
NAMES = ["ID", *[f"C{column:03}" for column in range(1, 255)]]
NATIVE = ["--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000"]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def model(mixed):
    result = {}
    for identity in range(1, 21):
        if identity in [6, 17]:
            continue
        updated = identity == 1
        width = 255 if updated or identity > 18 else WIDTHS[(identity - 1) // 2]
        row = [f"I32:{identity}"]
        for column in range(1, 255):
            kind = column % 5 if mixed else 4
            absent = column >= width or (identity + column) % 7 == 0
            if kind == 0:
                row.append("B1" if not absent and (identity + column) % 2 == 0 else "B0")
            elif absent:
                row.append("N")
            elif kind == 1:
                row.append(f"U{(identity + column) % 256}")
            elif kind == 2:
                row.append(f"I16:{identity * 100 - column}")
            elif kind == 3:
                row.append(f"I32:{identity * 100_000 - column}")
            else:
                text = "é😀" if updated else ["", "a", "é", "中", "😀"][(identity + column) % 5]
                row.append("T" + text.encode("utf-8").hex())
        result[identity] = row
    assert len(result) == 18
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed engineering artifact"
    generated = {}
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            path = args.directory / f"wide-{profile}.{suffix}"
            subprocess.run([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "WideTableHistoryFixture.java"), str(path), profile,
            ], check=True)
            generated[profile] = digest(path)
        inventory = {
            "recipe_sha256": digest(RECIPES / "WideTableHistoryFixture.java"),
            "jar_sha256": JAR_SHA256,
            "inputs": generated,
        }
        (args.directory / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")

    controls = 0
    for profile, suffix in PROFILES:
        path = args.directory / f"wide-{profile}.{suffix}"
        before = digest(path)
        assert before == (generated or INPUT_HASHES)[profile], "Unregistered input generation"
        for table in ["WideVariable", "WideMixed"]:
            expected = model(table == "WideMixed")
            reference = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), table,
            ], text=True, encoding="utf-8")
            records = [line.split("\t")[1:] for line in reference.splitlines()]
            identities = [int(row[0][4:]) for row in records]
            assert len(identities) == len(set(identities)) == len(expected)
            assert all(row == expected[identity] for identity, row in zip(identities, records)), (profile, table, "model")
            controls += 1

            for mode in [[], ["--batch-size", "1"], ["--batch-size", "7"]]:
                actual = subprocess.check_output([
                    str(args.rows), str(path), table, "--bytes", "67108864",
                    "--steps", "10000000", *mode,
                ], text=True, encoding="utf-8")
                assert actual == reference, (profile, table, mode, "tagged read")
                controls += 1

            selected = [0, 7, 8, 15, 16, 126, 127, 128, 253, 254]
            wanted = "".join(table + "\t" + "\t".join(row[column] for column in selected) + "\n" for row in records)
            actual = subprocess.check_output([
                str(args.rows), str(path), table, "--batch-size", "3", "--bytes", "67108864",
                "--steps", "10000000",
                *[part for column in reversed(selected) for part in ["--column", NAMES[column]]],
            ], text=True, encoding="utf-8")
            assert actual == wanted, (profile, table, "projected native-order read")
            controls += 1

            ordered = "".join(
                table + "\t" + "\t".join(expected[identity]) + "\n"
                for identity in sorted(expected)
            )
            indexed_reference = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), "--index", table, "IdAsc",
            ], text=True, encoding="utf-8")
            indexed_actual = subprocess.check_output([
                str(args.rows), str(path), table, "--index", "IdAsc", "--bytes", "67108864",
                "--steps", "10000000",
            ], text=True, encoding="utf-8")
            assert indexed_reference == indexed_actual == ordered, (profile, table, "index order")
            controls += 1

            def command(arguments, wanted=None, fails=False):
                nonlocal controls
                result = subprocess.run([str(args.cli), *map(str, arguments)], capture_output=True, text=True, encoding="utf-8")
                if fails:
                    assert result.returncode == 2 and not result.stdout, (profile, table, "buffered refusal", result.stderr)
                else:
                    assert result.returncode == 0 and not result.stderr, (profile, table, result.stderr)
                    actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                    assert actual == wanted, (profile, table, arguments[:3], "display")
                controls += 1

            displayed = [[display(tag) for tag in row] for row in records]
            command(["rows", path, table, "--limit", "100"], [NAMES, *displayed])
            command(["rows", path, table, "--stream", "--limit", "7", *NATIVE], [NAMES, *displayed])
            descending = sorted(expected, reverse=True)
            wanted = [NAMES, *[[display(tag) for tag in expected[identity]] for identity in descending]]
            for index in [[], ["--index", table, "IdAsc"]]:
                command(["query", "run", f"SELECT * FROM {table} ORDER BY ID DESC", "--database", path, *NATIVE, *index], wanted)
                selected_names = [NAMES[column] for column in reversed(selected)]
                sql = f"SELECT {','.join(selected_names)} FROM {table} WHERE ID BETWEEN 2 AND 18 ORDER BY ID"
                selected_ids = sorted(identity for identity in expected if 2 <= identity <= 18)
                wanted_projection = [selected_names, *[
                    [display(expected[identity][column]) for column in reversed(selected)]
                    for identity in selected_ids
                ]]
                command(["query", "run", sql, "--database", path, *NATIVE, *index], wanted_projection)

            command([
                "query", "run", f"SELECT Count(*) AS N,Count(C127) AS A,Count(C254) AS B FROM {table}",
                "--database", path, *NATIVE,
            ], [["N", "A", "B"], [str(len(expected)), *[
                str(sum(row[column] != "N" for row in records)) for column in [127, 254]
            ]]])
            command(["query", "run", f"SELECT * FROM {table}", "--database", path, "--native-bytes", "1"], fails=True)
        assert digest(path) == before, "Original changed"
        print(f"{profile}: {path.stat().st_size} bytes; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} wide-table history controls; native Access unrun.")


if __name__ == "__main__":
    main()
