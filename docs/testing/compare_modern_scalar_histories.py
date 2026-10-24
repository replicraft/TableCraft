"""Compare ACE 16 modern scalar histories with inert getters and Python models.

Only TableCraft executes SQL. Native Access save/reopen and SQL qualification
remain unrun. Original fixture bytes must remain unchanged.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_mixed_scalar_histories import EXECUTION, NATIVE, binary, text
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2016": "d17fa71f3283bb50b6f3571926c18c842024aa8a37c1d28640f980dd23ded1e2",
    "V2019": "03bc1d98ac89b903dc3d40caa7c65edd6b31f29d93f386a40a26719f36e98acb",
}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def wide(identity):
    return [
        -(2 ** 63) + identity,
        2 ** 63 - 1 - identity,
        2 ** 53 + identity,
        -(2 ** 53) - identity,
        -(2 ** 31) - identity,
        2 ** 31 - 1 + identity,
        identity * 19,
    ][identity % 7]


def stamp(identity, ticks):
    # Explicit year spelling also covers year 0001 independently of strftime.
    bases = ["0001-01-01 00:00:00", "9999-12-31 23:59:59",
             "2000-02-29 12:34:56", "2024-02-29 01:02:03"]
    return "Z" + bases[identity % 4] + f".{identity + ticks:07d}"


def model(extended):
    positions = {position * 7_919 % 20_000 + 1: position for position in range(20_000)}
    result = {}
    for identity in range(1, 20_101):
        appended = identity > 20_000
        if not appended and identity % 101 == 0:
            continue
        updated = not appended and identity % 97 == 0
        later = appended or updated or positions[identity] >= 10_000
        absent = identity % 13 == 0
        number = -wide(identity) if updated else wide(identity)
        note = f"grown:{identity}:" + "é😀" * 128 if updated else f"n:{identity}"
        row = [
            f"I32:{identity}", "N" if absent else f"I64:{number}",
            "N" if absent else text(f"label:{identity}:é中😀"),
            "N" if absent else text(note),
            "N" if absent else binary(identity, 4_097 if updated else identity % 67),
        ]
        row.extend("B1" if identity & (1 << bit) else "B0" for bit in range(9))
        if extended:
            row.append("N" if absent else stamp(identity, 1 if updated else 0))
        row.extend([
            f"I64:{-number}" if later and not absent else "N",
            text(("updated:" if updated else "later:") + str(identity)) if later and not absent else "N",
            "B1" if later and identity % 3 == 0 else "B0",
        ])
        if extended:
            row.append(stamp(identity, 9) if later and not absent else "N")
        result[identity] = row
    assert len(result) == 19_902
    return result


def columns(extended):
    names = ["ID", "Wide", "Label", "Notes", "Opaque"]
    names.extend(f"Flag{bit}" for bit in range(9))
    if extended:
        names.append("Stamp")
    names.extend(["LaterWide", "LaterText", "LaterFlag"])
    if extended:
        names.append("LaterStamp")
    return names


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "rows", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    controls = 0

    for profile, family in [("V2016", 5), ("V2019", 6)]:
        extended = profile == "V2019"
        path = args.directory / f"modern-{profile}.accdb"
        before = digest(path)
        assert before == INPUT_HASHES[profile], (profile, "Unregistered fixture")
        with path.open("rb") as source:
            assert source.read(24)[20:24] == bytes([family, 0, 0, 0])
        expected = model(extended)
        names = columns(extended)

        def getter(*arguments):
            return subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "TableDataOracle.java"), str(path), *arguments,
            ], text=True, encoding="utf-8")

        reference = getter("ModernHistory")
        records = [line.split("\t")[1:] for line in reference.splitlines()]
        identities = [int(row[0][4:]) for row in records]
        assert len(identities) == len(set(identities)) == len(expected)
        for identity, row in zip(identities, records):
            assert row == expected[identity], (profile, identity, "independent values", row, expected[identity])
        controls += 1

        for batch in [127, 257]:
            actual = subprocess.check_output([
                str(args.rows), str(path), "ModernHistory", "--batch-size", str(batch),
                "--bytes", "268435456", "--steps", "100000000",
            ], text=True, encoding="utf-8")
            assert actual == reference, (profile, batch, "complete cursor")
            controls += 1
        selected = [names.index(name) for name in ["Wide", "Opaque", "LaterWide", "LaterFlag"]]
        if extended:
            selected.extend([names.index("Stamp"), names.index("LaterStamp")])
        selected.sort()
        actual = subprocess.check_output([
            str(args.rows), str(path), "ModernHistory", "--batch-size", "131",
            "--bytes", "268435456", "--steps", "100000000",
            *[argument for position in reversed(selected) for argument in ["--column", names[position]]],
        ], text=True, encoding="utf-8")
        wanted = "".join("ModernHistory\t" + "\t".join(row[position] for position in selected) + "\n" for row in records)
        assert actual == wanted, (profile, "projected cursor")
        controls += 1

        def command(arguments, wanted=None, diagnostic=None):
            nonlocal controls
            result = subprocess.run([str(args.cli), *map(str, arguments)], capture_output=True, text=True, encoding="utf-8")
            if diagnostic:
                assert result.returncode == 2 and not result.stdout, (profile, "non-atomic refusal")
                assert diagnostic in result.stderr, (profile, result.stderr)
            else:
                assert result.returncode == 0 and not result.stderr, (profile, result.stderr)
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual == wanted, (profile, arguments[:3], "display mismatch")
            controls += 1

        displayed = [[display(tag) for tag in row] for row in records]
        command(["rows", path, "ModernHistory", "--stream", "--limit", "257", *NATIVE], [names, *displayed])
        command(["rows", path, "ModernHistory", "--offset", "19900", "--limit", "9"], [names, *displayed[19900:]])

        def query(sql, wanted=None, options=(), diagnostic=None):
            command(["query", "run", sql, "--database", path, *NATIVE, *EXECUTION, *options], wanted, diagnostic)

        def projected_rows(identities, selected_names):
            positions = [names.index(name) for name in selected_names]
            return [selected_names, *[
                [display(expected[identity][position]) for position in positions]
                for identity in identities
            ]]

        descending = sorted(expected, reverse=True)
        query(
            "SELECT TOP 17 * FROM ModernHistory ORDER BY ID DESC",
            projected_rows(descending[:17], names),
        )
        grown = [
            identity for identity in descending
            if identity <= 20_000 and identity % 97 == 0
        ][:17]
        query(
            "SELECT TOP 17 * FROM ModernHistory "
            "WHERE ID<=20000 AND ID Mod 97=0 ORDER BY ID DESC",
            projected_rows(grown, names),
        )
        query(
            "SELECT Count(*) AS N,Count(Wide) AS W,"
            "Count(LaterWide) AS L,Sum(ID) AS S FROM ModernHistory",
            [["N", "W", "L", "S"], [
                str(len(expected)),
                str(sum(row[1] != "N" for row in records)),
                str(sum(row[names.index("LaterWide")] != "N" for row in records)),
                str(sum(expected)),
            ]],
        )

        # The independent index getter establishes physical order; Python checks
        # all keys and complete identity coverage before that order is reused.
        index_reference = getter("--index", "ModernHistory", "WideAsc")
        indexed = [line.split("\t")[1:] for line in index_reference.splitlines()]
        index_ids = [int(row[0][4:]) for row in indexed]
        assert len(index_ids) == len(set(index_ids)) == len(expected)
        keys = [None if row[1] == "N" else int(row[1][4:]) for row in indexed]
        assert keys == sorted(keys, key=lambda value: (value is not None, value or 0))
        assert all(
            row == expected[identity]
            for identity, row in zip(index_ids, indexed)
        )
        controls += 1
        query(
            "SELECT ID,Wide FROM ModernHistory",
            projected_rows(index_ids, ["ID", "Wide"]),
            ["--index", "ModernHistory", "WideAsc"],
        )

        def selected_number(identity):
            value = expected[identity][1]
            return None if value == "N" else int(value[4:])

        for selection in [[], ["--index", "ModernHistory", "WideAsc"]]:
            predicates = [
                ("Wide IS NULL", lambda identity: selected_number(identity) is None),
                (
                    "Wide>=9007199254740993 AND Wide<=9007199254741010",
                    lambda identity: selected_number(identity) is not None
                    and 9_007_199_254_740_993 <= selected_number(identity)
                    <= 9_007_199_254_741_010,
                ),
                (
                    "Wide=-9007199254740995",
                    lambda identity: selected_number(identity) == -9_007_199_254_740_995,
                ),
            ]
            for predicate, include in predicates:
                matches = [identity for identity in sorted(expected) if include(identity)]
                query(
                    f"SELECT ID,Wide FROM ModernHistory WHERE {predicate} ORDER BY ID",
                    projected_rows(matches, ["ID", "Wide"]),
                    selection,
                )

        if extended:
            position = names.index("Stamp")
            ordered = sorted(
                expected,
                key=lambda identity: (
                    expected[identity][position] != "N",
                    expected[identity][position],
                ),
                reverse=True,
            )
            query(
                "SELECT TOP 17 ID,Stamp,LaterStamp "
                "FROM ModernHistory ORDER BY Stamp DESC,ID",
                projected_rows(ordered[:17], ["ID", "Stamp", "LaterStamp"]),
            )

        command(["query", "run", "SELECT Count(*) FROM ModernHistory", "--database", path, *EXECUTION], diagnostic="limit")
        assert digest(path) == before, (profile, "Original changed")
        print(f"{profile}: {len(expected)} exact modern history rows; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} modern scalar history controls; native Access unrun.")


if __name__ == "__main__":
    main()
