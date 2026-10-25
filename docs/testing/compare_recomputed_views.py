"""Compare explicit recomputed native views with inert ordinary-getter models.

Existing registered originals are never authored or changed. Only TableCraft
executes SQL; these controls do not qualify Microsoft Access calculation results.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from compare_calculated_dependencies import getters, model
from compare_progressive_queries import display

ROOT = Path(__file__).resolve().parents[2]
PROFILES = {
    "V2016": "87b70b3e8182981966a6a77a022c37d463d7ef049c6c99e2c0e04bcf1618e150",
    "V2019": "f3a8c0a7e3c842b6bfa9b492e5409935c337d6f24e2bbecd06f4fcb3395dce6d",
}
JAR_HASH = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
SOURCE_ROOTS = [
    "crates/backends/tablecraft-access/src/source/recomputed",
    "crates/backends/tablecraft-access/src/source/calculation.rs",
    "crates/backends/tablecraft-access/src/source/calculation",
    "crates/query/tablecraft-query/src/binding/named.rs",
    "crates/query/tablecraft-query/src/execution/domain",
    "apps/tablecraft-cli/src/commands/query/recomputed.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/run_args/dispatch.rs",
]
VIEW_COLUMNS = ["ID", "Input", "Total", "Diamond", "Title"]
MODES = {
    "materialized": [],
    "declared": ["--declared-arithmetic"],
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def invoke(command):
    arguments = [relative(value) if isinstance(value, Path) else str(value) for value in command]
    return subprocess.run(arguments, capture_output=True, text=True, timeout=120, cwd=ROOT)


def source_pins(revision):
    listing = invoke(["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS])
    listing.check_returncode()
    pins = {}
    for name in listing.stdout.splitlines():
        source = subprocess.run(
            ["git", "show", f"{revision}:{name}"],
            capture_output=True,
            check=True,
            timeout=120,
            cwd=ROOT,
        )
        pins[name] = hashlib.sha256(source.stdout).hexdigest()
    if not any("/recomputed/" in name for name in pins):
        raise RuntimeError("Compiled revision does not contain recomputed views")
    return pins


def selection(alias="Fresh", table="Chains", columns=VIEW_COLUMNS):
    flags = []
    for column in columns:
        flags.extend(["--recomputed-column", alias, table, column])
    return flags


def success_cases(records, mode):
    direct = [
        [row["ID"], model("Total", row), model("Diamond", row), model("Title", row)]
        for row in records
    ]
    yield (
        "direct",
        "SELECT ID,Total,Diamond,Title,VarType(Total) AS Carrier FROM Fresh ORDER BY ID",
        ["ID", "Total", "Diamond", "Title", "Carrier"],
        [
            [*row, 1 if row[1] is None else 3 if mode == "declared" else 20]
            for row in direct
        ],
        selection(),
    )
    filtered = [row[:2] for row in direct if row[1] is not None and row[1] > 0]
    yield (
        "where",
        "SELECT ID,Total FROM Fresh WHERE Total>0 ORDER BY ID",
        ["ID", "Total"],
        filtered,
        selection(),
    )
    groups = {}
    for row in records:
        key = model("Total", row)
        groups[key] = groups.get(key, 0) + 1
    grouped = [[key, count] for key, count in groups.items()]
    yield (
        "groups",
        "SELECT Total,Count(*) AS N FROM Fresh GROUP BY Total",
        ["Total", "N"],
        grouped,
        selection(),
    )
    yield (
        "join",
        "SELECT F.ID,F.Diamond FROM Fresh F INNER JOIN Chains C ON F.ID=C.ID ORDER BY F.ID",
        ["ID", "Diamond"],
        [[row[0], row[2]] for row in direct],
        selection(),
    )
    numeric = [[row[1]] for row in direct] + [[row["Input"]] for row in records]
    yield (
        "numeric-union",
        "SELECT Total AS V FROM Fresh UNION ALL SELECT [Input] FROM Chains",
        ["V"],
        numeric,
        selection(),
    )
    text = [[row[3]] for row in direct] + [[row["Label"]] for row in records]
    yield (
        "text-union",
        "SELECT Title AS V FROM Fresh UNION ALL SELECT Label FROM Chains",
        ["V"],
        text,
        selection(),
    )
    second = next(row for row in records if row["ID"] == 2)
    yield (
        "domain",
        "SELECT DLookup('Diamond','Fresh','ID=2') AS V FROM Chains WHERE ID=1",
        ["V"],
        [[model("Diamond", second)]],
        selection(),
    )
    derived = [[row[0], None if row[1] is None else row[1] + row[2]] for row in direct]
    yield (
        "derived-alias",
        "SELECT ID,Total+Diamond AS Combined FROM (SELECT ID,Total,Diamond FROM Fresh) Q ORDER BY ID",
        ["ID", "Combined"],
        derived,
        selection(),
    )
    empty = selection("Empty", "EmptyChains", ["ID", "Diamond"])
    yield (
        "empty",
        "SELECT ID,Diamond FROM Empty",
        ["ID", "Diamond"],
        [],
        empty,
    )
    empty_error = selection("Empty", "EmptyChains", ["LateError"])
    yield (
        "empty-arithmetic",
        "SELECT LateError FROM Empty",
        ["LateError"],
        [],
        empty_error,
    )
    caches = [[row["ID"], row["Total"], row["Diamond"], row["Title"]] for row in records]
    yield (
        "stored-caches",
        "SELECT ID,Total,Diamond,Title FROM Chains ORDER BY ID",
        ["ID", "Total", "Diamond", "Title"],
        caches,
        ["--stored-calculations"],
    )
    both = [[row["ID"], row["Total"], model("Total", row)] for row in records]
    yield (
        "cached-and-fresh",
        "SELECT C.ID,C.Total AS Cached,F.Total AS FreshValue FROM Chains C INNER JOIN Fresh F ON C.ID=F.ID ORDER BY C.ID",
        ["ID", "Cached", "FreshValue"],
        both,
        ["--stored-calculations", *selection()],
    )


def refusal_cases():
    late = selection("Bad", "Chains", ["ID", "LateError"])
    yield (
        "eager-false",
        "SELECT ID FROM Bad WHERE False",
        late,
        "division by zero",
        {},
    )
    yield (
        "eager-top",
        "SELECT TOP 1 ID FROM Bad",
        late,
        "division by zero",
        {},
    )
    yield (
        "ordinary-cache-route",
        "SELECT Total FROM Chains",
        [],
        "",
        {},
    )
    yield (
        "captured-alias",
        "SELECT ID FROM Chains",
        selection("Chains", "Chains", ["ID"]),
        "",
        {},
    )
    yield (
        "missing-column",
        "SELECT ID FROM Fresh",
        selection(columns=["Missing"]),
        "",
        {},
    )
    for label, option, diagnostic in [
        ("native-quota", "--native-rows", "resource limit"),
        ("execution-quota", "--execution-steps", "resource limit"),
        ("output-quota", "--output-bytes", "output limit"),
    ]:
        yield (
            label,
            "SELECT ID,Diamond FROM Fresh",
            selection(),
            diagnostic,
            {option: "1"},
        )


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.observations = []
        self.compared_rows = 0
        self.compared_cells = 0

    def check(self, database, mode, label, sql, flags, headings=None, rows=None, refusal=None, overrides=None):
        limits = {
            "--native-rows": "10000",
            "--execution-rows": "100000",
            "--execution-bytes": "67108864",
            "--execution-steps": "10000000",
        }
        limits.update(overrides or {})
        command = [
            self.cli, "--format", "tsv", "query", "run", sql,
            "--database", database,
            "--number-format", "invariant",
            "--collation", "ordinal",
            *MODES[mode], *flags,
        ]
        for option, value in limits.items():
            command.extend([option, value])
        result = invoke(command)
        observation = {
            "profile": database.stem,
            "mode": mode,
            "case": label,
            "command": [relative(value) if isinstance(value, Path) else value for value in command],
            "exit_code": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
        if refusal is not None:
            matches = result.returncode != 0 and not result.stdout and refusal in result.stderr.lower()
            observation["expected_refusal"] = refusal or "Nonzero status and empty stdout"
        else:
            expected = [headings, *[[display(value) for value in row] for row in rows]]
            actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
            matches = result.returncode == 0 and not result.stderr and actual == expected
            observation["expected"] = expected
            if matches:
                self.compared_rows += len(rows)
                self.compared_cells += sum(map(len, rows))
        observation["matches"] = matches
        self.observations.append(observation)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Refusing to overwrite evidence")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_HASH:
        raise RuntimeError("Executable/JAR identity mismatch")
    # Reject report destinations outside this checkout before recording paths.
    relative(args.report)
    generation_path = args.directory / "generation.json"
    generation = json.loads(generation_path.read_text())
    compiled = ROOT / generation["compiled_class"]
    if digest(compiled) != generation["compiled_class_sha256"]:
        raise RuntimeError("Registered getter class identity mismatch")
    paths = {
        args.cli, args.jar, generation_path, compiled, Path(__file__),
        ROOT / "docs/testing/CalculatedDependencyFixture.java",
        ROOT / "docs/testing/compare_calculated_dependencies.py",
        ROOT / "docs/testing/compare_progressive_queries.py",
    }
    for profile, expected in PROFILES.items():
        database = args.directory / (profile + ".accdb")
        if digest(database) != expected:
            raise RuntimeError("Registered original identity mismatch")
        paths.update([database, database.with_suffix(".accdb.getters.tsv")])
    pins = {relative(path): digest(path) for path in sorted(paths)}
    sources = source_pins(args.revision)
    getter_observations = []
    replay = Replay(args.cli)
    for profile in PROFILES:
        database = args.directory / (profile + ".accdb")
        command = [
            "java", "-cp", relative(args.jar) + ":" + relative(compiled.parent),
            "CalculatedDependencyFixture", "read", database,
        ]
        result = invoke(command)
        if result.returncode or result.stderr or result.stdout != database.with_suffix(".accdb.getters.tsv").read_text():
            raise RuntimeError("Inert getter output differs from registered observation")
        getter_observations.append({
            "command": [relative(value) if isinstance(value, Path) else value for value in command],
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "exit_code": result.returncode,
        })
        records = getters(database.with_suffix(".accdb.getters.tsv"))["Chains"]["rows"]
        for mode in MODES:
            for label, sql, headings, rows, flags in success_cases(records, mode):
                replay.check(database, mode, label, sql, flags, headings, rows)
            for label, sql, flags, refusal, overrides in refusal_cases():
                replay.check(database, mode, label, sql, flags, refusal=refusal, overrides=overrides)
    after = {name: digest(ROOT / name) for name in pins}
    if pins != after or sources != source_pins(args.revision):
        raise RuntimeError("An immutable input or compiled source pin changed")
    disagreements = sum(not item["matches"] for item in replay.observations)
    report = {
        "scope": __doc__,
        "compiled_revision": args.revision,
        "compiled_source_sha256": sources,
        "hashes_before": pins,
        "hashes_after": after,
        "getter_observations": getter_observations,
        "controls": len(replay.observations),
        "successful_controls": sum("expected" in item for item in replay.observations),
        "intended_refusals": sum("expected_refusal" in item for item in replay.observations),
        "compared_rows": replay.compared_rows,
        "compared_cells": replay.compared_cells,
        "disagreements": disagreements,
        "observations": replay.observations,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({key: report[key] for key in [
        "controls", "successful_controls", "intended_refusals",
        "compared_rows", "compared_cells", "disagreements",
    ]}))
    if disagreements:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
