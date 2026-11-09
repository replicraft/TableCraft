"""Independent wide-variable/Memo getter replay against an immutable CLI.

Source hashes are verified against a committed archive, not the moving worktree.
Reference getters never execute SQL; large outputs are compared fully then hashed.
"""

import argparse
import base64
from collections import Counter
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess
import tarfile

from compare_saved_external import display


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019")
CLI_SHA256 = "40fc7f58e5bfa48458762bfcc97b1b1ea3232ac6c7b10d4a16a15f83cbfb292e"
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
MANIFEST_SHA256 = "8f377ddf7734b22c74ba71f79defbd12ddd116d93c5bbeacb86ddda084e50541"
SELECTED = (0, 15, 126, 127, 253)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def archive_sources(revision, manifest):
    entries = dict(line.split("\0") for line in manifest.read_text().splitlines())
    complete = subprocess.check_output(["git", "rev-parse", revision], text=True).strip()
    process = subprocess.Popen(["git", "archive", "--format=tar", complete], stdout=subprocess.PIPE)
    seen = set()
    with tarfile.open(fileobj=process.stdout, mode="r|") as archive:
        for member in archive:
            if member.name not in entries:
                continue
            assert member.isfile(), member.name
            source = archive.extractfile(member)
            actual = hashlib.sha256(source.read()).hexdigest()
            assert actual == entries[member.name], member.name
            seen.add(member.name)
    assert process.wait() == 0
    assert seen == entries.keys()
    return {"revision": complete, "rust_files_verified": len(seen),
            "model": "immutable git archive bytes; no live worktree source claim"}


def read_getters(path):
    columns = []
    physical = {}
    rows = []
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "COLUMNS":
            assert fields[1] == "255"
        elif fields[0] == "COLUMN":
            columns.append(fields[1:])
        elif fields[0] == "PHYSICAL":
            physical[int(fields[1])] = tuple(map(int, fields[2:]))
        elif fields[0] == "ROW":
            cells = [None if value == "N" else base64.b64decode(value[1:]).decode("utf-8") for value in fields[2:]]
            rows.append((int(fields[1]), cells))
        else:
            raise AssertionError(fields)
    assert columns[0] == ["ID", "LONG"]
    assert len(columns) == 255 and len(rows) == 32
    for index, (name, kind) in enumerate(columns[1:]):
        assert name == f"V{index:03}" and kind == ("MEMO" if index % 31 == 0 else "TEXT")
    assert Counter(value[0] for value in physical.values()) == {17: 1, 128: 1, 129: 1, 255: 29}
    assert [physical[index][0] for index in (1, 9, 17, 25)] == [17, 128, 129, 255]
    assert all(len(cells) == 254 for _, cells in rows)
    return columns, rows, physical


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "source_manifest", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--source-revision", default="3fbf14ea")
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == CLI_SHA256
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.source_manifest) == MANIFEST_SHA256
    archived = archive_sources(args.source_revision, args.source_manifest)
    paths = {args.cli, args.jar, args.source_manifest, Path(__file__)}
    paths.update(args.directory.iterdir())
    paths.update(ROOT / "docs/testing" / name for name in (
        "VariableMemoHistoryFixture.java", "compare_saved_external.py",
    ))
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths) if path.is_file()}
    observations = []
    profiles = []
    matched_cells = 0

    def execute(label, database, sql, expected=None, index=False, refusal=None, extra=()):
        nonlocal matched_cells
        command = [str(args.cli), "--format", "tsv", "query", "run", "--database", str(database),
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        if index:
            command.extend(["--index", "VariableHistory", "PrimaryKey"])
        # Replace a base ceiling when testing an explicitly smaller allowance.
        for option, value in extra:
            if option in command:
                command[command.index(option) + 1] = value
            else:
                command.extend([option, value])
        command.append(sql)
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        cells = 0
        if refusal:
            assert result.returncode != 0 and not result.stdout, (label, result.stdout[:100], result.stderr)
            assert refusal in result.stderr, (label, refusal, result.stderr)
        else:
            assert result.returncode == 0, (label, result.stderr)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert actual == expected, label
            cells = sum(len(row) for row in expected[1:])
            matched_cells += cells
        observations.append({"label": label, "command": command, "exit": result.returncode,
                             "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
                             "stdout_bytes": len(result.stdout.encode()), "matched_cells": cells,
                             "stderr": result.stderr, "expected_refusal": refusal})

    for profile in PROFILES:
        database = args.directory / (profile + ".accdb")
        columns, rows, physical = read_getters(database.with_suffix(".accdb.getters.tsv"))
        complete = [[column[0] for column in columns], *[
            [str(identity), *map(display, cells)] for identity, cells in rows
        ]]
        selected = [["ID", *[f"V{index:03}" for index in SELECTED]], *[
            [str(identity), *[display(cells[index]) for index in SELECTED]] for identity, cells in rows
        ]]
        for indexed in (False, True):
            execute(profile + ":full:" + str(indexed), database,
                    "SELECT * FROM VariableHistory ORDER BY ID", complete, indexed)
            execute(profile + ":projected:" + str(indexed), database,
                    "SELECT " + ",".join(selected[0]) + " FROM VariableHistory ORDER BY ID", selected, indexed)
        tail = sum(cells[253] is not None for _, cells in rows)
        longest = max(len(cells[0].encode("utf-16-le")) // 2 for _, cells in rows if cells[0] is not None)
        middle_null = sum(cells[127] is None for _, cells in rows)
        execute(profile + ":aggregate", database,
                "SELECT Count(*) AS N,Count(V253) AS Tail,Max(Len(V000)) AS LongUnits,"
                "Sum(IIf(V127 IS NULL,1,0)) AS NullMiddle FROM VariableHistory",
                [["N", "Tail", "LongUnits", "NullMiddle"], ["32", str(tail), str(longest), str(middle_null)]])
        execute(profile + ":historical-tail", database,
                "SELECT ID FROM VariableHistory WHERE V253 IS NULL ORDER BY ID",
                [["ID"], *[[str(identity)] for identity, cells in rows if cells[253] is None]])
        execute(profile + ":step-refusal", database, "SELECT * FROM VariableHistory",
                refusal="resource limit", extra=(("--native-steps", "1"),))
        execute(profile + ":output-refusal", database, "SELECT * FROM VariableHistory",
                refusal="output limit", extra=(("--output-bytes", "1"),))
        profiles.append({"profile": profile, "rows": len(rows), "columns": len(columns), "memo_columns": 9,
                         "physical_row_field_counts": dict(Counter(value[0] for value in physical.values())),
                         "root_pages": len({value[1] for value in physical.values()}),
                         "longest_memo_utf16_units": longest})

    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    report = {"scope": "Original ACE16 wide-variable/Memo history engineering evidence; "
                       "no native Access qualification.",
              "controls": len(observations), "matched_cells": matched_cells,
              "archived_sources": archived, "profiles": profiles, "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "matched_cells": matched_cells,
                      "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
