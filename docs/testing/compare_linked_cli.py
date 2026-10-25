"""Replay actual explicit CLI links against independent ordinary getter records.

Uses the accepted engineering inputs unchanged, plus public-writer lowercase
ForeignName additions. No stored Database value selects a file to open.
"""

import argparse
import csv
from datetime import datetime, timezone
import io
import json
from pathlib import Path
import subprocess

from compare_linked_queries import PROFILES, ROOT, cases, digest, java, model, value

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def display(cell):
    if cell is None:
        return "NULL"
    if isinstance(cell, str):
        escaped = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
        text = "".join(escaped.get(character, character) for character in cell)
        return '"' + text + '"'
    return str(cell)


def command(cli, frontend, backend, sql, aliases, comparison="ordinal", extra=()):
    arguments = [cli, "query", "run"]
    arguments.extend([sql] if sql is not None else ["--saved", "Linked root"])
    arguments.extend(["--database", frontend, "--format", "tsv", "--collation", comparison])
    for alias in aliases:
        arguments.extend(["--link", alias, backend])
    return [*arguments, *extra]


def execute(arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True,
                            text=True, encoding="utf-8", timeout=240)
    assert result.returncode == 0, (arguments, result.stderr)
    return list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))


def refused(arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True, timeout=240)
    assert result.returncode != 0 and not result.stdout, (arguments, result.stderr)
    return result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "engineering_report", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--generate-case-copies", action="store_true")
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    engineering = json.loads(args.engineering_report.read_text())
    for name, expected in engineering["hashes"].items():
        assert digest(ROOT / name) == expected, name
    paths = {
        args.cli, args.jar, Path(__file__), args.engineering_report,
        ROOT / "docs/testing/compare_linked_queries.py",
        ROOT / "docs/testing/LinkedQueryFixture.java",
        ROOT / "docs/testing/LinkedQueryOracle.java",
        ROOT / "docs/testing/LinkedQueryCaseFixture.java",
        *[ROOT / "apps/tablecraft-cli/src" / name for name in (
            "commands/query/links.rs", "commands/query/native_options.rs",
            "commands/query/retrieval.rs", "commands/query/run_args.rs", "native.rs",
        )],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    cells = 0
    profiles = []
    for profile in PROFILES:
        directory = args.directory / profile
        root_format = "V2000" if profile == "Mixed" else profile
        target_format = "V2016" if profile == "Mixed" else profile
        frontend = directory / ("frontend.mdb" if root_format == "V2000" else "frontend.accdb")
        backend = directory / ("backend.mdb" if target_format == "V2000" else "backend.accdb")
        before = {path: digest(path) for path in (frontend, backend)}
        records = [line.split("\t") for line in java(
            args.jar, "LinkedQueryOracle.java", frontend, backend,
        ).splitlines()]
        links, _, tables, policies, mappings = model(records)
        aliases = [mapping[0] for mapping in mappings]
        for comparison in ("ordinal", "database"):
            for name, sql, columns, expected in cases(tables, comparison):
                actual = execute(command(args.cli, frontend, backend, sql, aliases, comparison))
                wanted = [columns, *[[display(cell) for cell in row] for row in expected]]
                assert actual == wanted, (profile, comparison, name, actual, wanted)
                controls += 1
                cells += sum(map(len, expected))
            sql = "SELECT p.ID,r.Team FROM [People link] p INNER JOIN Rates r " \
                  "ON p.Label=r.Label ORDER BY p.ID,r.Team"
            if comparison == "database" and policies[("Remote People", "Label")] != policies[("Rates", "Label")]:
                refused(command(args.cli, frontend, backend, sql, aliases, comparison))
            else:
                fold = (lambda text: text) if comparison == "ordinal" else str.lower
                expected = [[person[0], rate[0]] for person in tables["Remote People"]
                            for rate in tables["Rates"] if person[2] is not None
                            and fold(person[2]) == fold(rate[2])]
                actual = execute(command(args.cli, frontend, backend, sql, aliases, comparison))
                assert actual == [["ID", "Team"], *[[display(cell) for cell in row] for row in expected]]
                cells += sum(map(len, expected))
            controls += 1
        local = "SELECT Team,Rate,Label FROM Rates ORDER BY Team"
        local_expected = [["Team", "Rate", "Label"], *[[display(cell) for cell in row] for row in tables["Rates"]]]
        assert execute(command(args.cli, frontend, backend, local, [])) == local_expected
        controls += 1
        for sql, chosen in (
                ("SELECT ID FROM [Unmapped link]", aliases),
                ("SELECT ID FROM [People link]", []),
                ("SELECT ID FROM [Remote People]", aliases),
                ("SELECT Missing FROM [People link]", aliases),
                ("SELECT 1/(ID-4) FROM [People link]", aliases)):
            refused(command(args.cli, frontend, backend, sql, chosen))
            controls += 1
        refused(command(args.cli, frontend, backend, local, [*aliases, aliases[0].swapcase()]))
        refused(command(args.cli, frontend, backend, local, ["Rates"]))
        refused(command(args.cli, frontend, backend, local, ["Unknown alias"]))
        controls += 3
        for extra in (["--native-bytes", "0"], ["--native-rows", "0"],
                      ["--native-steps", "0"], ["--execution-rows", "0"],
                      ["--output-bytes", "0"], ["--index", "People link", "Missing index"]):
            refused(command(args.cli, frontend, backend, "SELECT ID FROM [People link]", aliases, extra=extra))
            controls += 1
        # Positive values pass argument admission and exercise runtime refusal.
        for extra in (["--native-bytes", "1"], ["--native-rows", "1"],
                      ["--native-steps", "1"], ["--execution-rows", "1"],
                      ["--output-bytes", "1"]):
            message = refused(command(args.cli, frontend, backend,
                "SELECT ID FROM [People link]", aliases, extra=extra))
            assert b"Usage:" not in message and b"invalid value" not in message.lower(), message
            controls += 1
        expected = [["ID"], *[[str(row[0])] for row in tables["Remote People"]]]
        assert execute(command(args.cli, frontend, backend, "SELECT ID FROM [People link] ORDER BY ID",
                               ["pEoPlE LiNk"])) == expected
        controls += 1
        for change in ("provider", "odbc", "chain"):
            copy = directory / f"control-{change}{frontend.suffix}"
            refused(command(args.cli, copy, backend, "SELECT ID FROM [People link]", ["People link"]))
            assert execute(command(args.cli, copy, backend, local, [])) == local_expected
            controls += 2
            hashes[str(copy.resolve().relative_to(ROOT))] = digest(copy)
        opaque = directory / f"control-database{frontend.suffix}"
        assert execute(command(args.cli, opaque, backend, "SELECT ID FROM [People link] ORDER BY ID",
                               ["People link"])) == expected
        controls += 1
        hashes[str(opaque.resolve().relative_to(ROOT))] = digest(opaque)
        lowercase = directory / f"lowercase-frontend{frontend.suffix}"
        if args.generate_case_copies:
            java(args.jar, "LinkedQueryCaseFixture.java", frontend, lowercase, backend.name)
        changed_records = [line.split("\t") for line in java(
            args.jar, "LinkedQueryOracle.java", lowercase, backend,
        ).splitlines()]
        added = [row for row in changed_records if row[0] == "L" and value(row[1]) == "Lowercase link"]
        assert len(added) == 1 and value(added[0][4]) == "remote people"
        assert changed_records[-1] == ["O", "0"]
        for alias in ("Lowercase link", "LOWERCASE LINK"):
            assert execute(command(args.cli, lowercase, backend,
                "SELECT ID FROM [Lowercase link] ORDER BY ID", [alias])) == expected
            controls += 1
        hashes[str(lowercase.resolve().relative_to(ROOT))] = digest(lowercase)
        for path, expected_hash in before.items():
            assert digest(path) == expected_hash, path
            hashes[str(path.resolve().relative_to(ROOT))] = expected_hash
        profiles.append({"profile": profile, "resolver_calls": 0,
                         "originals_unchanged": True, "stored_foreign_name_case": "remote people"})
    for name, expected_hash in hashes.items():
        assert digest(ROOT / name) == expected_hash, name
    for name, expected_hash in engineering["hashes"].items():
        assert digest(ROOT / name) == expected_hash, name
    args.report.write_text(json.dumps({
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "controls": controls, "main_compared_result_cells": cells,
        "positive_runtime_limit_refusals": len(PROFILES) * 5,
        "zero_argument_or_limit_refusals": len(PROFILES) * 5,
        "hashes": hashes, "profiles": profiles,
        "scope": "Actual CLI with explicit host paths; no native linked SQL or path-identity qualification",
    }, indent=2) + "\n")
    print(f"Passed {controls} actual CLI link controls, {cells} main result cells, {len(hashes)} unchanged hashes")


if __name__ == "__main__":
    main()
