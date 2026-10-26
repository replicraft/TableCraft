"""Compare original getter membership with explicit native ignore-NULL reads."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2000", "V2007", "V2016", "V2019")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
BASELINE_SHA256 = "1093091eb45bd7e78fe1f0536da4994439e7012682b1f19f358ca13330cb52a0"
SOURCE_PATHS = (
    "crates/engines/tablecraft-ace/src/table/indexes/profile.rs",
    "crates/engines/tablecraft-ace/src/table/indexes/read.rs",
    "crates/engines/tablecraft-ace/src/table/indexes/check.rs",
    "crates/engines/tablecraft-ace/src/table/indexes/check/ignored_nulls.rs",
    "crates/engines/tablecraft-ace/src/table/mutation/gates.rs",
    "crates/backends/tablecraft-access/src/source/selection/automatic.rs",
)
ORIGINAL_SHA256 = {
    "V2000": "6f608a93375a3ef88941b4e5c7f7bb2922709a9ba2272e6a8750002ae3fd0114",
    "V2007": "d3add63c222c3ce805bbac8188ba8791c65c8e4fcba022cadb8d53fd5fca9875",
    "V2016": "1b7afd89440039b877ae55e700dd07c0bb0e0ed7fab6a5909aaf9073f3489a16",
    "V2019": "6246f75593d6ca7a7b9f71f7009cbf1627d8b50eef9504f6df14fba8b9122898",
}
INDEX_COLUMNS = {
    "IgnoredAsc": "key",
    "IgnoredAlias": "key",
    "IgnoredDesc": "key",
    "UniqueIgnored": "unique",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def pin(path):
    data = Path(path).read_bytes()
    return {"path": label(path), "bytes": len(data), "sha256": digest(data)}


def parse_getter(text):
    rows = []
    entries = {name: [] for name in INDEX_COLUMNS}
    metadata = {}
    for fields in csv.reader(io.StringIO(text), delimiter="\t"):
        if fields[0] == "TABLE":
            table = {
                "declared_rows": int(fields[2]),
                "definition_page": int(fields[3]),
                "page_bytes": int(fields[4]),
            }
        elif fields[0] == "ROW":
            values = [None if value == "NULL" else int(value) for value in fields[2:]]
            rows.append({"id": int(fields[1]), "key": values[0], "unique": values[1]})
        elif fields[0] == "INDEX":
            metadata[fields[1]] = {
                "physical": int(fields[2]),
                "entries": int(fields[3]),
                "owned_pages": int(fields[4]),
                "counter": int(fields[5]),
                "unique": fields[6] == "true",
                "ignore_nulls": fields[7] == "true",
            }
        elif fields[0] == "ENTRY":
            entries[fields[1]].append(int(fields[2]))

    assert len(rows) == 701 and len({row["id"] for row in rows}) == 701
    by_id = {row["id"]: row for row in rows}
    encounter = {row["id"]: position for position, row in enumerate(rows)}
    for name, column in INDEX_COLUMNS.items():
        eligible = [row for row in rows if row[column] is not None]
        # Actual numeric values and encounter positions come exclusively from getters.
        # Equal-key reference order is separately captured by the public index cursor.
        values = [by_id[identifier][column] for identifier in entries[name]]
        assert all(value is not None for value in values)
        assert set(entries[name]) == {row["id"] for row in eligible}
        assert len(entries[name]) == len(eligible) == metadata[name]["entries"]
        assert values == sorted(values, reverse=name == "IgnoredDesc")
        assert metadata[name]["ignore_nulls"] and metadata[name]["owned_pages"] > 1
        if name == "UniqueIgnored":
            assert len(set(values)) == len(values) and metadata[name]["unique"]
        else:
            for left, right in zip(entries[name], entries[name][1:]):
                if by_id[left][column] == by_id[right][column]:
                    assert encounter[left] < encounter[right]
    assert metadata["IgnoredAsc"]["physical"] == metadata["IgnoredAlias"]["physical"]
    return {"rows": rows, "entries": entries, "metadata": metadata, "table": table}


class Replay:
    def __init__(self, log):
        self.log = log
        self.controls = []
        self.disagreements = []

    def run(self, arguments, expected_rows=None, contains=(), refused=False):
        command = [str(argument) for argument in arguments]
        completed = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=90, check=False)
        stdout = completed.stdout.decode("utf-8")
        stderr = completed.stderr.decode("utf-8")
        if refused:
            agrees = completed.returncode != 0 and stdout == ""
            agrees = agrees and stderr == "error: query feature is not implemented\n"
        else:
            agrees = completed.returncode == 0 and stderr == ""
            agrees = agrees and all(fragment in stdout for fragment in contains)
            if expected_rows is not None:
                actual = list(csv.reader(io.StringIO(stdout), delimiter="\t"))
                agrees = agrees and actual == [["ID"], *[[str(value)] for value in expected_rows]]
        record = {
            "ordinal": len(self.controls) + 1,
            "command": command,
            "exit_code": completed.returncode,
            "stdout": stdout,
            "stderr": stderr,
            "expected_rows": expected_rows,
            "expected_fragments": list(contains),
            "expected_refusal": refused,
            "agrees": agrees,
        }
        self.controls.append(record)
        if not agrees:
            self.disagreements.append(record["ordinal"])
        with self.log.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record) + "\n")


def source_objects(revision):
    result = {}
    for path in SOURCE_PATHS:
        data = subprocess.run(
            ["git", "show", f"{revision}:{path}"], cwd=ROOT,
            capture_output=True, check=True, timeout=30,
        ).stdout
        result[path] = {"bytes": len(data), "sha256": digest(data)}
    return result


def raw_nodes(directory, profile, model):
    data = (directory / f"Ignore-{profile}.accdb").read_bytes()
    text = (directory / f"{profile}-baseline-indexes.tsv").read_text(encoding="utf-8")
    definitions = list(csv.reader(io.StringIO(text), delimiter="\t"))[2:]
    result = []
    width = model["table"]["page_bytes"]
    owner = model["table"]["definition_page"]
    for fields in definitions:
        root = int(fields[3])
        page = data[root * width:(root + 1) * width]
        assert page[:2] == b"\x03\x01"
        assert int.from_bytes(page[4:8], "little") == owner
        assert fields[5] == ("0x83" if fields[0] == "UniqueIgnored" else "0x82")
        result.append({"index": fields[0], "root": root, "kind": page[0], "sha256": digest(page)})
    assert len(result) == 4
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument(
        "--baseline-cli", type=Path,
        default=ROOT / ".cache/tablecraft-bin/inspect-native-work-cli-20261025",
    )
    parser.add_argument(
        "--originals", type=Path,
        default=ROOT / ".cache/tablecraft-fixtures/ignore-null-indexes-20261025",
    )
    parser.add_argument("--jar", type=Path, default=ROOT / ".cache/tablecraft-fixtures/jackcess-5.0.4.jar")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists() and not args.log.exists(), "Accepted outputs must be fresh"
    assert digest(args.cli.read_bytes()) == args.cli_sha256
    assert digest(args.baseline_cli.read_bytes()) == BASELINE_SHA256
    assert digest(args.jar.read_bytes()) == JAR_SHA256

    tools = [
        args.cli, args.baseline_cli, args.jar, Path(__file__),
        ROOT / "docs/testing/IgnoreNullIndexFixture.java",
    ]
    for profile in PROFILES:
        tools.extend(args.originals / f"{profile}{suffix}" for suffix in ("-getter.tsv", "-baseline-indexes.tsv"))
        original = args.originals / f"Ignore-{profile}.accdb"
        assert digest(original.read_bytes()) == ORIGINAL_SHA256[profile]
        tools.append(original)
    before = [pin(path) for path in tools]
    selected_sources = source_objects(args.revision)
    live_native_before = {path: pin(ROOT / path) for path in SOURCE_PATHS}
    for path, source in selected_sources.items():
        assert live_native_before[path]["sha256"] == source["sha256"]
        assert live_native_before[path]["bytes"] == source["bytes"]
    replay = Replay(args.log)
    observations = {}
    for profile in PROFILES:
        path = args.originals / f"Ignore-{profile}.accdb"
        model = parse_getter((args.originals / f"{profile}-getter.tsv").read_text(encoding="utf-8"))
        observations[profile] = {
            "actual_getter_rows": len(model["rows"]),
            "indexes": model["metadata"],
            "raw_node_roots": raw_nodes(args.originals, profile, model),
        }
        rows = model["rows"]
        by_id = {row["id"]: row for row in rows}
        common = [label(args.cli), "--format", "tsv"]
        baseline = [label(args.baseline_cli), "--format", "tsv"]
        check_args = ["inspect", label(path), "--table", "Ignored", "--check", "--native-steps", "10000000"]
        replay.run(
            [*common, *check_args],
            contains=("Rows checked\t701", "Indexes checked\t4", "Indexes not checked\t0"),
        )
        replay.run(
            [*baseline, *check_args],
            contains=("Rows checked\t701", "Indexes checked\t0", "Indexes not checked\t4"),
        )

        def query(sql, index=None, automatic=False, executable=common, expected_rows=None, refused=False):
            arguments = [*executable, "query", "run", sql, "--database", label(path)]
            if index is not None:
                arguments.extend(("--index", "Ignored", index))
            if automatic:
                arguments.append("--auto-index")
            replay.run(arguments, expected_rows=expected_rows, refused=refused)

        for name, column in INDEX_COLUMNS.items():
            native_column = "KeyValue" if column == "key" else "UniqueValue"
            expected = [identifier for identifier in model["entries"][name] if by_id[identifier][column] == 0]
            query(f"SELECT ID FROM Ignored WHERE {native_column}=0", index=name, expected_rows=expected)
            expected = [
                identifier for identifier in model["entries"][name]
                if -12 <= by_id[identifier][column] < 9
            ]
            query(
                f"SELECT ID FROM Ignored WHERE {native_column}>=-12 AND {native_column}<9",
                index=name, expected_rows=expected,
            )
        query("SELECT ID FROM Ignored WHERE KeyValue=0", automatic=True,
              expected_rows=[row["id"] for row in rows if row["key"] == 0])
        query("SELECT ID FROM Ignored WHERE KeyValue>=-12 AND KeyValue<9", automatic=True,
              expected_rows=[row["id"] for row in rows if row["key"] is not None and -12 <= row["key"] < 9])
        query("SELECT ID FROM Ignored WHERE KeyValue IS NULL", automatic=True,
              expected_rows=[row["id"] for row in rows if row["key"] is None])
        query("SELECT ID FROM Ignored", index="IgnoredAsc", refused=True)
        query("SELECT ID FROM Ignored WHERE KeyValue IS NULL", index="IgnoredAsc", refused=True)
        query("SELECT ID FROM Ignored WHERE KeyValue=0", index="IgnoredAsc", executable=baseline, refused=True)

    after = [pin(path) for path in tools]
    assert before == after, "Immutable tools or originals changed"
    assert selected_sources == source_objects(args.revision)
    live_native_after = {path: pin(ROOT / path) for path in SOURCE_PATHS}
    assert live_native_before == live_native_after
    report = {
        "scope": "Original public-writer/getter engineering comparison; no Microsoft Access qualification",
        "native_source_revision": args.revision,
        "selected_native_source_objects": selected_sources,
        "selected_native_live_before": live_native_before,
        "selected_native_live_after": live_native_after,
        "build_scope_limitation": (
            "Frozen CLI hash identifies the tested executable. Selected native source "
            "objects match this revision and the live files before/after. The full "
            "checkout contained unrelated query/format edits when this CLI was built; "
            "this is not a claim that the complete executable was built from a clean "
            "checkout of the native source revision."
        ),
        "immutable_pins_before": before,
        "immutable_pins_after": after,
        "observations": observations,
        "controls": len(replay.controls),
        "disagreements": replay.disagreements,
        "raw_log": pin(args.log),
        "results": replay.controls,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({
        "controls": report["controls"], "disagreements": report["disagreements"],
        "report": label(args.report),
    }))
    return bool(replay.disagreements)


if __name__ == "__main__":
    raise SystemExit(main())
