"""Compare complete physical checks with original nullable-key getter populations."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019")
CURRENT_SHA256 = "9ef8530041ddefff1b888b6b58757a35221297044497fa9a32360570c1ed259f"
BASELINE_SHA256 = "121d76588ed4e6c9497e23b9f42c6f9ca5e729aceb0a0e9491f153865fac3e4c"
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ORIGINAL_SHA256 = {
    "V2016": "90c5fc95e47be4996924ac6b0fe232a5209a12689527c5c05be593ec0b7f08df",
    "V2019": "2db1bd079e3f9df28dc82c2d3eb9dc82753a8a75273b67213eefa5e2ce872b05",
}
UNQUALIFIED_UNIQUE = frozenset(("UniqueDigest", "UniqueMEMO0", "UniqueMEMO1"))
OLD_IGNORED_TABLES = frozenset((
    "ScalarINT32", "UniqueINT32", "ScalarINT64", "UniqueINT64",
    "ScalarCURRENCY", "UniqueCURRENCY",
))
ROW_OFFSET_MASK = 0x1fff
ROW_FLAG_MASK = 0xe000


def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def pin(path):
    data = Path(path).read_bytes()
    return {"path": label(path), "bytes": len(data), "sha256": digest(data)}


def getter_model(text, original):
    tables = {}
    for fields in csv.reader(io.StringIO(text), delimiter="\t"):
        tag, name = fields[:2]
        if tag == "TABLE":
            tables[name] = {
                "declared_rows": int(fields[2]), "definition_page": int(fields[3]),
                "page_bytes": int(fields[4]), "columns": [], "rows": {},
                "indexes": {}, "locations": [],
            }
        elif tag == "COLUMN":
            tables[name]["columns"].append(fields[2])
        elif tag == "ROW":
            table = tables[name]
            identifier = int(fields[2])
            assert identifier not in table["rows"]
            table["rows"][identifier] = dict(zip(table["columns"][1:], fields[3:]))
        elif tag == "INDEX":
            tables[name]["indexes"][fields[2]] = {
                "physical": int(fields[3]), "declared_entries": int(fields[4]),
                "owned_pages": int(fields[5]), "counter": int(fields[6]),
                "unique": fields[7] == "true", "ignore_nulls": fields[8] == "true",
                "keys": [], "entries": [],
            }
        elif tag == "KEY":
            tables[name]["indexes"][fields[2]]["keys"].append((fields[3], fields[4]))
        elif tag == "ENTRY":
            tables[name]["indexes"][fields[2]]["entries"].append(int(fields[3]))
        elif tag == "LOCATION":
            tables[name]["locations"].append(tuple(map(int, fields[2:])))

    populations = []
    for name, table in tables.items():
        rows = table["rows"]
        assert len(rows) == table["declared_rows"]
        for index_name, index in table["indexes"].items():
            # Membership uses actual getter NULLs, never index counters or key hashing.
            expected = {
                identifier for identifier, row in rows.items()
                if not index["ignore_nulls"]
                or any(row[column] != "NULL" for column, _ in index["keys"])
            }
            actual = index["entries"]
            assert len(actual) == len(set(actual)) == len(expected)
            assert set(actual) == expected
            assert len(actual) == index["declared_entries"]
            populations.append({
                "table": name, "index": index_name, "rows": len(rows),
                "eligible_roots": len(expected), "physical": index["physical"],
                "owned_pages": index["owned_pages"], "unique": index["unique"],
                "ignore_nulls": index["ignore_nulls"],
            })
        if "IgnoredAlias" in table["indexes"]:
            assert table["indexes"]["Ignored"]["physical"] == table["indexes"]["IgnoredAlias"]["physical"]

    evolved = tables["EvolvedPair"]
    histories = []
    for identifier, page, slot, row_start in evolved["locations"]:
        page_start = page * evolved["page_bytes"]
        offset = page_start + row_start + slot * 2
        entry = int.from_bytes(original[offset:offset + 2], "little")
        assert entry & ROW_FLAG_MASK == 0
        row = page_start + (entry & ROW_OFFSET_MASK)
        columns = int.from_bytes(original[row:row + 2], "little")
        expected_columns = 1 if identifier < 17 else 3
        assert columns == expected_columns
        histories.append({"id": identifier, "stored_columns": columns, "page": page, "slot": slot})
    assert len(histories) == 21
    assert len(evolved["indexes"]["Ignored"]["entries"]) == 3
    assert len(tables) == 43
    assert sum(len(table["rows"]) for table in tables.values()) == 10393
    assert len(populations) == 171
    return tables, populations, histories


def expected_check(table_name, table, baseline):
    statuses = {}
    for name, index in table["indexes"].items():
        qualified = table_name not in UNQUALIFIED_UNIQUE
        if baseline and index["ignore_nulls"]:
            qualified = qualified and table_name in OLD_IGNORED_TABLES
        statuses[name] = (
            "Checked" if qualified else "Not checked: unsupported profile",
            "Checked: exact unique/required" if qualified else "Not checked",
        )
    return statuses


def check_output(stdout, table_name, table, expected):
    records = list(csv.reader(io.StringIO(stdout), delimiter="\t"))
    checked = sum(status[0] == "Checked" for status in expected.values())
    assert records[:5] == [
        ["Table", table_name], ["Rows checked", str(len(table["rows"]))],
        ["Indexes checked", str(checked)],
        ["Indexes not checked", str(len(expected) - checked)],
        ["Index", "Status", "Index constraints"],
    ]
    actual = {row[0]: tuple(row[1:]) for row in records[5:]}
    assert actual == expected, (actual, expected)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--baseline-cli", type=Path, required=True)
    parser.add_argument("--originals", type=Path, required=True)
    parser.add_argument("--build-record", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists() and not args.log.exists()
    assert digest(args.cli.read_bytes()) == CURRENT_SHA256
    assert digest(args.baseline_cli.read_bytes()) == BASELINE_SHA256
    jar = ROOT / ".cache/tablecraft-fixtures/jackcess-5.0.4.jar"
    assert digest(jar.read_bytes()) == JAR_SHA256
    build = json.loads(args.build_record.read_text())
    native_sources = build["dirty_production_sources"]
    for path, expected in native_sources.items():
        assert pin(ROOT / path)["sha256"] == expected["sha256"]
    artifacts = [
        args.cli, args.baseline_cli, jar, Path(__file__), args.build_record,
        ROOT / "docs/testing/IgnoredIndexCoverageFixture.java",
        args.originals / "source-at-generation.java",
        args.originals / "producer-getter-commands.json",
        args.originals / "location-getter-commands.json",
    ]
    for profile in PROFILES:
        artifacts.extend((
            args.originals / f"IgnoredProfiles-{profile}.accdb",
            args.originals / f"{profile}-getter.stdout",
            args.originals / f"{profile}-getter-locations.stdout",
            args.originals / f"{profile}-generate.stderr",
        ))
    before = [pin(path) for path in artifacts]
    results = []
    populations = {}
    histories = {}
    with args.log.open("x", encoding="utf-8") as log:
        for profile in PROFILES:
            original_path = args.originals / f"IgnoredProfiles-{profile}.accdb"
            original = original_path.read_bytes()
            assert digest(original) == ORIGINAL_SHA256[profile]
            text = (args.originals / f"{profile}-getter-locations.stdout").read_text()
            tables, populations[profile], histories[profile] = getter_model(text, original)
            for name, table in tables.items():
                for baseline in (False, True):
                    executable = args.baseline_cli if baseline else args.cli
                    command = [
                        str(executable), "--format", "tsv", "inspect", str(original_path),
                        "--table", name, "--check", "--native-bytes", "134217728",
                        "--native-steps", "20000000",
                    ]
                    actual = subprocess.run(command, capture_output=True, text=True, timeout=60)
                    error = None
                    try:
                        assert actual.returncode == 0 and not actual.stderr
                        check_output(actual.stdout, name, table, expected_check(name, table, baseline))
                    except AssertionError as failure:
                        error = str(failure) or "Exit/status/header mismatch"
                    observation = {
                        "profile": profile, "table": name, "baseline": baseline,
                        "command": command, "exit_code": actual.returncode,
                        "stdout": actual.stdout, "stderr": actual.stderr,
                        "agrees": error is None, "disagreement": error,
                    }
                    log.write(json.dumps(observation) + "\n")
                    results.append(observation)
    after = [pin(path) for path in artifacts]
    assert before == after
    for path, expected in native_sources.items():
        assert pin(ROOT / path)["sha256"] == expected["sha256"]
    report = {
        "scope": "Independent complete-table check statuses and getter-derived physical memberships, not new public seeks or native Access integrity qualification",
        "controls": len(results), "disagreements": sum(not row["agrees"] for row in results),
        "candidate_checks": sum(not row["baseline"] for row in results),
        "baseline_checks": sum(row["baseline"] for row in results),
        "tables_per_profile": 43, "actual_getter_rows_per_profile": 10393,
        "logical_populations_per_profile": 171,
        "physical_populations": populations, "historical_row_headers": histories,
        "immutable_artifacts_before_after": before,
        "selected_native_source_snapshots": native_sources,
        "binary_build_revision": build["revision"],
        "build_checkout_limitation": "CLI built with eight pending native source snapshots; revision alone does not identify the entire compiled checkout. Snapshot hashes and frozen executable identify tested behavior.",
        "raw_log": pin(args.log),
        "observations": results,
        "excluded": [
            "Initial read-only candidate sweep is diagnostic, not counted here.",
            "Public-writer generation emitted retained duplicate-null-entry warnings; actual cursor populations independently match all getter-derived eligible sets.",
            "Generation source snapshot differs only by later getter-only LOCATION output/import; original producer methods unchanged.",
            "Unique Memo/digest profiles retain Unsupported; old baseline retains Unsupported for other ignored profiles.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(results), "disagreements": report["disagreements"], "report": pin(args.report)}))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
