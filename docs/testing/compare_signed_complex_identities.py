"""Replay persisted signed child IDs through inert native collection views.

This compares original independent-writer files and getter observations, not
native Access SQL, write permission or helper-sentinel insertion semantics.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PROFILES = ("V2016", "V2019")
IDENTITIES = (1, -2147483648, -7, -2, -1, 0, 7, 2147483647)
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=240
    )
    assert result.returncode == 0, (arguments, result.stderr)
    return result.stdout


def tsv(text):
    return list(csv.reader(io.StringIO(text), delimiter="\t", strict=True))


def text(value):
    return '"' + value + '"'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    paths = {
        args.cli, args.jar, Path(__file__),
        RECIPES / "SignedComplexIdentityFixture.java",
        RECIPES / "SignedComplexIdentityOracle.java",
        *[args.directory / f"{profile}.accdb" for profile in PROFILES],
        *[ROOT / "crates/engines/tablecraft-ace/src/table/complex" / name for name in (
            "links/membership.rs", "attachments/record.rs", "history/record.rs",
            "attachments/metadata_only/read.rs", "attachments/read.rs", "history/read.rs",
        )],
        ROOT / "crates/backends/tablecraft-access/src/source/collections/load.rs",
    }
    hashes = {
        str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)
    }
    controls = 0
    replays = []
    for profile in PROFILES:
        path = args.directory / f"{profile}.accdb"
        records = [line.split("\t") for line in execute(
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            RECIPES / "SignedComplexIdentityOracle.java", path,
        ).splitlines()]
        assert records[0] == ["S", "-1", "null-row-location"]
        hidden, history_text, history_time = next(row[1:] for row in records if row[0] == "D")
        for member in ("Values", "Files", hidden):
            raw = [row for row in records if row[:2] == ["R", member]]
            assert [int(row[2]) for row in raw] == list(IDENTITIES)
            assert all(row[3] == "1" for row in raw)
        assert [row for row in records if row[0] == "P"] == [
            ["P", "1", "8", "8", "8"], ["P", "2", "0", "0", "0"]
        ]
        getters = {}
        for tag in ("M", "A", "H"):
            getters[tag] = {
                int(row[2]): row[3:] for row in records if row[0] == tag
            }
            assert set(getters[tag]) == set(IDENTITIES)
        controls += 1

        for flag, member in (
                ("--complex-links", "Values"), ("--complex-links", "Files"),
                ("--complex-links", hidden), ("--attachments", "Files"),
                ("--version-history", "Notes")):
            inspected = tsv(execute(
                args.cli, "inspect", path, "--table", "Identities",
                flag, member, "--format", "tsv",
            ))
            if flag == "--complex-links":
                assert inspected[-2:] == [["0", "1", "8"], ["1", "2", "0"]]
            else:
                heading = next(index for index, row in enumerate(inspected)
                               if row[:2] == ["Parent ordinal", "Identity"])
                assert [int(row[1]) for row in inspected[heading + 1:]] == list(IDENTITIES)
            controls += 1

        numbers = [["1", getters["M"][identity][0]] for identity in IDENTITIES]
        names = [["1", text(getters["A"][identity][0])] for identity in IDENTITIES]
        histories = [["1", text(getters["H"][identity][0])] for identity in IDENTITIES]
        date = "#2024-01-01 00:00:00.000000000#"
        cases = [
            ("SELECT ID,[Values].Value FROM Identities", numbers + [["2", "NULL"]], []),
            ("SELECT ID,Files.FileName FROM Identities", names + [["2", "NULL"]], []),
            (f"SELECT ID,[{hidden}].[{history_text}] FROM Identities", histories + [["2", "NULL"]], []),
            (f"SELECT [{hidden}].[{history_time}] FROM Identities", [[date]] * 8 + [["NULL"]], []),
            ("SELECT Files.FileName,Files.FileData FROM Identities",
             [[text(getters["A"][identity][0]), "0x" + getters["A"][identity][1]]
              for identity in IDENTITIES] + [["NULL", "NULL"]], []),
            ("SELECT Files.FileName,Files.FileData FROM Identities",
             [[text(getters["A"][identity][0]), "0x" + getters["A"][identity][2]]
              for identity in IDENTITIES] + [["NULL", "NULL"]], ["--decoded-attachments"]),
            ("SELECT Count(*) FROM Identities WHERE Files IS NOT NULL", [["1"]], []),
            ("SELECT Count([Values].Value) FROM Identities", [["7"]], []),
            ("SELECT ID FROM Identities WHERE [Values].Value IS NULL", [["1"]], []),
            ("SELECT Count(*) FROM Identities", [["2"]], []),
            ("SELECT Count(*) FROM Identities WHERE ID=99", [["0"]], []),
            ("SELECT TOP 1 Files.FileName FROM Identities", [[names[0][1]]], []),
            (f"SELECT Count([{hidden}].[{history_text}]) FROM Identities", [["8"]], []),
            ("SELECT ID FROM Identities WHERE Files IS NULL", [["2"]], []),
            (f"SELECT ID FROM Identities WHERE [{hidden}] IS NULL", [["2"]], []),
            ("SELECT ID,Files.FileName FROM Identities WHERE ID=2", [["2", "NULL"]], []),
        ]
        for collation in ("ordinal", "database"):
            for sql, expected, extra in cases:
                result = tsv(execute(
                    args.cli, "query", "run", sql, "--database", path,
                    "--collections", "--collation", collation, "--format", "tsv", *extra,
                ))
                assert result[1:] == expected, (profile, collation, sql, result, expected)
                controls += 1
                replays.append({"profile": profile, "collation": collation,
                                "sql": sql, "options": extra, "rows": len(expected)})
            for extra in (["--native-rows", "0"], ["--execution-rows", "0"],
                          ["--output-bytes", "0"]):
                failed = subprocess.run(list(map(str, [
                    args.cli, "query", "run", "SELECT Files.FileData FROM Identities",
                    "--database", path, "--collections", "--collation", collation,
                    "--format", "tsv", *extra,
                ])), capture_output=True, timeout=240)
                assert failed.returncode != 0 and not failed.stdout
                controls += 1
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    args.report.write_text(json.dumps({
        "date": "2026-10-24", "controls": controls, "profiles": PROFILES,
        "identities": IDENTITIES, "hashes": hashes, "queries": replays,
        "qualification": "Independent persisted reads only; native Access remains unqualified",
    }, indent=2) + "\n")
    print(f"Passed {controls} signed collection identity controls; {len(hashes)} unchanged hashes")


if __name__ == "__main__":
    main()
