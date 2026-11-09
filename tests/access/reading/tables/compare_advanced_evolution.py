"""Compare evolved advanced-field reads with original inert independent getters.

This checks stored physical rows and explicit TableCraft collection/query models,
not Access recalculation, concurrency, native SQL or preservation after saves.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from compare_browsing import display
from compare_cached_pages import date_display
from compare_calculated_decimals import digest, run

ROOT = Path(__file__).resolve().parents[2]
ORACLE = ROOT / "docs/testing/AdvancedEvolutionOracle.java"
FIXTURE = ROOT / "docs/testing/AdvancedEvolutionFixture.java"
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
FIELDS = [
    "ID", "Label", "Ordinary", "Cached", "Notes", "LaterMemo", "Amount",
    *[f"Flag{bit}" for bit in range(8)],
]
SOURCE_FILES = [
    "crates/engines/tablecraft-ace/src/table/record.rs",
    "crates/engines/tablecraft-ace/src/table/page/cursor.rs",
    "crates/engines/tablecraft-ace/src/table/calculated/projection.rs",
    "crates/engines/tablecraft-ace/src/table/calculated/encoding.rs",
    "crates/engines/tablecraft-ace/src/table/complex/links/read.rs",
    "crates/engines/tablecraft-ace/src/table/complex/multivalue/schema.rs",
    "crates/engines/tablecraft-ace/src/table/complex/attachments/schema.rs",
    "crates/engines/tablecraft-ace/src/table/complex/history/schema.rs",
    "crates/backends/tablecraft-access/src/source/schema.rs",
    "crates/backends/tablecraft-access/src/source/projection.rs",
]


def grid(names, rows):
    def cell(value):
        return date_display(value) if value.startswith("Y64:") else display(value)

    body = "".join("\t".join(cell(value) for value in row) + "\n" for row in rows)
    return "\t".join(names) + "\n" + body


def physical_layouts(original, positions):
    layouts = Counter()
    relocated = 0
    for page, slot in positions:
        seen = set()
        while True:
            assert (page, slot) not in seen
            seen.add((page, slot))
            directory = page * 4096 + 14 + slot * 2
            entry = int.from_bytes(original[directory:directory + 2], "little")
            start = page * 4096 + (entry & 0x1FFF)
            if len(seen) == 1:
                assert not entry & 0x8000
            # Overflow targets can carry the hidden/deleted slot bit while
            # remaining reachable through a live root. They are not new roots.
            if not entry & 0x4000:
                break
            relocated += 1
            reference = int.from_bytes(original[start:start + 4], "little")
            page, slot = reference >> 8, reference & 255
        layouts[int.from_bytes(original[start:start + 2], "little")] += 1
    return dict(sorted(layouts.items())), relocated


def records(output):
    parents, choices, files, histories, positions = [], [], [], [], []
    declaration = None
    for line in output.splitlines():
        cells = line.split("\t")
        tag = cells[0]
        if tag == "D":
            declaration = cells[1:]
        elif tag == "R":
            assert int(cells[1]) == len(parents)
            assert len(cells[2:]) == len(FIELDS)
            parents.append(cells[2:])
            choices.append([])
            files.append([])
            histories.append([])
        elif tag == "P":
            assert int(cells[1]) == len(positions)
            positions.append(tuple(map(int, cells[2:])))
        elif tag == "C":
            choices[int(cells[1])].append(cells[3])
        elif tag == "A":
            files[int(cells[1])].append(cells[3:])
        elif tag == "H":
            histories[int(cells[1])].append(cells[3:])
        else:
            raise AssertionError("Unknown original oracle record")
    return parents, choices, files, histories, positions, declaration


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256
    paths = [
        args.jar, args.cli, ORACLE, FIXTURE, Path(__file__),
        args.ace2016, args.ace2019,
    ]
    paths.extend(ROOT / path for path in SOURCE_FILES)
    helper_names = (
        "compare_browsing.py", "compare_cached_pages.py",
        "compare_calculated_decimals.py", "cli_tsv.py",
    )
    paths.extend(ROOT / "docs/testing" / name for name in helper_names)
    hashes = {str(path): digest(path) for path in paths}
    controls = 0
    compared_cells = 0
    observations = []

    def java(*arguments):
        return run(
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            ORACLE, *arguments,
        )

    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        oracle = java(path)
        parents, choices, files, histories, positions, declaration = records(oracle)
        assert len(parents) == 154 and len(positions) == 154
        assert declaration is not None
        layouts, relocated = physical_layouts(original, positions)
        assert layouts == {8: 7, 18: 147} and relocated == 120
        assert sum(map(len, choices)) == 303
        assert sum(map(len, files)) == 130
        assert sum(map(len, histories)) == 30
        controls += 5
        ordinary = [
            "--database", path,
            "--native-steps", "100000000",
            "--execution-steps", "100000000",
        ]
        cached = [*ordinary, "--stored-calculations"]
        collections = [*ordinary, "--collections"]
        mixed = [*collections, "--stored-calculations"]

        def check(sql, names, rows, options):
            nonlocal controls, compared_cells
            expected = grid(names, rows)
            try:
                actual = run(args.cli, "query", "run", sql, *options)
            except AssertionError as error:
                raise AssertionError((path, sql, str(error))) from error
            assert actual == expected, (path, sql)
            controls += 1
            compared_cells += sum(map(len, rows))

        def refuse(sql, options):
            nonlocal controls
            command = [args.cli, "query", "run", sql, *options]
            result = subprocess.run(list(map(str, command)), capture_output=True)
            assert result.returncode == 2 and not result.stdout, (sql, result.stderr)
            controls += 1

        selections = [
            [0], [0, 1, 2, 7, 14], list(range(len(FIELDS))),
            [0, 3], [0, 5, 6],
        ]
        for selected in selections:
            names = [FIELDS[index] for index in selected]
            rows = [[row[index] for index in selected] for row in parents]
            sql = "SELECT " + ",".join("[" + name + "]" for name in names)
            sql += " FROM Evolution"
            needs_cache = any(index in selected for index in (3, 5, 6))
            check(sql, names, rows, cached if needs_cache else ordinary)

        for auto in [False, True]:
            rows = [
                [row[0], row[3]] for row in parents
                if row[3] != "N" and int(row[3].split(":")[1]) > 999100
            ]
            options = [*cached, *(["--auto-index"] if auto else [])]
            check(
                "SELECT ID,Cached FROM Evolution WHERE Cached>999100",
                ["ID", "Cached"], rows, options,
            )
        refuse("SELECT Cached FROM Evolution", ordinary)
        refuse(
            "SELECT Cached FROM Evolution",
            [*cached, "--index", "Evolution", "Missing"],
        )

        rows = [
            [parent[0], value] for parent, values in zip(parents, choices)
            for value in values or ["N"]
        ]
        check(
            "SELECT ID,Choices.Value AS Choice FROM Evolution",
            ["ID", "Choice"], rows, collections,
        )
        rows = [
            [parent[0], parent[3], value]
            for parent, values in zip(parents, choices)
            for value in values or ["N"]
        ]
        check(
            "SELECT ID,Cached,Choices.Value AS Choice FROM Evolution",
            ["ID", "Cached", "Choice"], rows, mixed,
        )
        rows = [
            [parent[0], "I64:" + str(sum(value != "N" for value in values))]
            for parent, values in zip(parents, choices)
        ]
        check(
            "SELECT ID,Count(Choices.Value) AS N FROM Evolution GROUP BY ID ORDER BY ID",
            ["ID", "N"], rows, collections,
        )
        rows = [
            [parent[0]] for parent, values in zip(parents, choices)
            if "N" in values and parent[3] == "N"
        ]
        check(
            "SELECT ID FROM Evolution WHERE Choices.Value IS NULL AND Cached IS NULL",
            ["ID"], rows, mixed,
        )

        for decoded in [False, True]:
            rows = [
                [parent[0], child[0], child[3 if decoded else 2]]
                for parent, values in zip(parents, files)
                for child in values or [["N"] * 4]
            ]
            options = [
                *collections, *(["--decoded-attachments"] if decoded else []),
            ]
            check(
                "SELECT ID,Files.FileName AS Name,Files.FileData AS Data FROM Evolution",
                ["ID", "Name", "Data"], rows, options,
            )
        rows = [
            [parent[0], parent[6], child[0]]
            for parent, values in zip(parents, files)
            for child in values or [["N"] * 4]
        ]
        check(
            "SELECT ID,Amount,Files.FileName AS Name FROM Evolution",
            ["ID", "Amount", "Name"], rows, mixed,
        )
        hidden, value_name, date_name = declaration
        rows = [
            [parent[0], parent[4], *child]
            for parent, values in zip(parents, histories)
            for child in values or [["N", "N"]]
        ]
        sql = (
            f"SELECT ID,Notes,[{hidden}].[{value_name}] AS Old,"
            f"[{hidden}].[{date_name}] AS Modified FROM Evolution"
        )
        check(sql, ["ID", "Notes", "Old", "Modified"], rows, collections)
        rows = [
            [parent[0], "I64:" + str(sum(child[0] != "N" for child in values))]
            for parent, values in zip(parents, histories)
        ]
        check(
            f"SELECT ID,Count([{hidden}].[{value_name}]) AS N "
            "FROM Evolution GROUP BY ID ORDER BY ID",
            ["ID", "N"], rows, collections,
        )

        with tempfile.TemporaryDirectory(prefix="tablecraft-advanced-evolution-") as directory:
            orphan = Path(directory) / "orphan.accdb"
            shutil.copyfile(path, orphan)
            java("--orphan", orphan)
            orphan_options = [*collections]
            orphan_options[1] = orphan
            refuse("SELECT Choices.Value FROM Evolution", orphan_options)
            refuse("SELECT TOP 1 Choices.Value FROM Evolution", orphan_options)
            check(
                "SELECT ID FROM Evolution", ["ID"],
                [[parent[0]] for parent in parents], orphan_options,
            )
            rows = [
                [parent[0], child[0]] for parent, values in zip(parents, files)
                for child in values or [["N"] * 4]
            ]
            check(
                "SELECT ID,Files.FileName AS Name FROM Evolution",
                ["ID", "Name"], rows, orphan_options,
            )
            chosen = next(row[3] for row in reversed(parents) if row[3] != "N")
            coefficient = int(chosen.split(":")[1]).to_bytes(4, "little", signed=True)
            needle = bytes(16) + (4).to_bytes(4, "little") + coefficient + bytes(3)
            assert original.count(needle) == 1
            changed = bytearray(original)
            changed[original.index(needle)] = 1
            bad = Path(directory) / "cache.accdb"
            bad.write_bytes(changed)
            bad_options = [*cached]
            bad_options[1] = bad
            refuse("SELECT TOP 1 Cached FROM Evolution", bad_options)
            refuse("SELECT Cached FROM Evolution WHERE ID=999", bad_options)
            check(
                "SELECT ID FROM Evolution", ["ID"],
                [[parent[0]] for parent in parents], bad_options,
            )
            assert bad.read_bytes() == changed
        observations.append({
            "file": str(path), "bytes": len(original), "sha256": digest(path),
            "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(),
            "parents": len(parents), "physical_field_counts": layouts,
            "relocation_links": relocated, "choices": sum(map(len, choices)),
            "attachments": sum(map(len, files)), "versions": sum(map(len, histories)),
        })

    assert all(digest(Path(path)) == expected for path, expected in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "compared_cells": compared_cells, "hashes_unchanged": True,
        "native_steps": 100_000_000, "execution_steps": 100_000_000,
        "inputs": {
            str(Path(path).resolve().relative_to(ROOT)): value
            for path, value in hashes.items()
        },
        "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(
        f"Passed {controls} advanced evolution controls, {compared_cells} stored query cells; "
        "originals and fixed executable unchanged"
    )


if __name__ == "__main__":
    main()
