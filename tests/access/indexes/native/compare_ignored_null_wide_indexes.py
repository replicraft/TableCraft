"""Exact Int64/Currency ignored-index replay over original getter records."""
import argparse
import csv
import io
import json
from pathlib import Path

from compare_ignored_null_indexes import (
    ROOT, Replay, SOURCE_PATHS, digest, label, pin, source_objects,
)

PROFILES = ("V2000", "V2007", "V2016", "V2019")
BASELINE_SHA256 = "1128c3a6e9b8a29bf667fea81d1c94809b6d7abe9566c9826e0ebd7f7bb24db7"
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ORIGINAL_SHA256 = {
    "V2000": "0d1abc65f3297a0724644f1ab35324d8ee37116d5195bcdb6a88da7f5df9b25f",
    "V2007": "9a1ef12af6df0b1b0075d9f1d0adfa9d5c62e6bf439a0604d82482ed19b43a40",
    "V2016": "ac37147c43e68f5e2a29dc0e839b41c9959b3b48b0801314ad954ee659c14f86",
    "V2019": "02bbfdbdf4172f0a527504d4ecfe73b0e78fa8937c1a3aeaf0070a05f6d81331",
}
INDEX_COLUMNS = {
    "IgnoredAsc": 0,
    "IgnoredAlias": 0,
    "IgnoredDesc": 0,
    "UniqueIgnored": 1,
}
POINTS = {
    "IgnoredAsc": -(1 << 63),
    "IgnoredAlias": (1 << 53) + 1,
    "IgnoredDesc": (1 << 63) - 1,
    "UniqueIgnored": 0,
}
LOWER = 1 << 53
UPPER = LOWER + 1
PAGE_OWNER_OFFSET = 4
PAGE_OWNER_BYTES = 4


def getter(text):
    tables = {}
    for fields in csv.reader(io.StringIO(text), delimiter="\t"):
        tag, name = fields[:2]
        if tag == "TABLE":
            tables[name] = {
                "declared_rows": int(fields[2]), "definition": int(fields[3]),
                "page_bytes": int(fields[4]), "type": fields[5],
                "rows": [], "entries": {}, "metadata": {},
            }
        elif tag == "ROW":
            tables[name]["rows"].append((
                int(fields[2]),
                tuple(None if value == "NULL" else int(value) for value in fields[3:]),
            ))
        elif tag == "INDEX":
            tables[name]["entries"][fields[2]] = []
            tables[name]["metadata"][fields[2]] = {
                "physical": int(fields[3]), "entries": int(fields[4]),
                "owned_pages": int(fields[5]), "counter": int(fields[6]),
                "unique": fields[7] == "true", "ignore_nulls": fields[8] == "true",
            }
        elif tag == "ENTRY":
            tables[name]["entries"][fields[2]].append(int(fields[3]))
        elif tag == "MONEY":
            # Independent text-to-scaled-units conversion, without binary floating point.
            assert currency_text(int(fields[5])) == fields[3]
            assert int(fields[4]) == 4

    for table in tables.values():
        rows = table["rows"]
        assert len(rows) == 701 and len({identifier for identifier, _ in rows}) == 701
        by_id = dict(rows)
        for name, column in INDEX_COLUMNS.items():
            ids = table["entries"][name]
            values = [by_id[identifier][column] for identifier in ids]
            assert all(value is not None for value in values)
            expected = {identifier for identifier, row in rows if row[column] is not None}
            assert set(ids) == expected and len(ids) == len(expected)
            assert len(ids) == table["metadata"][name]["entries"]
            assert values == sorted(values, reverse=name == "IgnoredDesc")
            assert table["metadata"][name]["owned_pages"] > 1
            assert table["metadata"][name]["ignore_nulls"]
            if name == "UniqueIgnored":
                assert len(set(values)) == len(values)
                assert table["metadata"][name]["unique"]
        assert table["metadata"]["IgnoredAsc"]["physical"] == table["metadata"]["IgnoredAlias"]["physical"]
    return tables


def currency_text(scaled):
    integral, fractional = divmod(abs(scaled), 10000)
    sign = "-" if scaled < 0 else ""
    return f"{sign}{integral}.{fractional:04d}"


def parameter(currency, name, value):
    kind = "currency" if currency else "bigint"
    text = currency_text(value) if currency else str(value)
    return f"{name}={kind}:{text}"


def raw_roots(directory, profile, name, table, original):
    text = (directory / f"{profile}-{name}-baseline-indexes.tsv").read_text(encoding="utf-8")
    definitions = list(csv.reader(io.StringIO(text), delimiter="\t"))[2:]
    width = table["page_bytes"]
    result = []
    for fields in definitions:
        root = int(fields[3])
        page = original[root * width:(root + 1) * width]
        assert page[:2] == b"\x03\x01"
        owner = int.from_bytes(page[PAGE_OWNER_OFFSET:PAGE_OWNER_OFFSET + PAGE_OWNER_BYTES], "little")
        assert owner == table["definition"]
        assert fields[5] == ("0x83" if fields[0] == "UniqueIgnored" else "0x82")
        result.append({"index": fields[0], "root": root, "sha256": digest(page)})
    assert len(result) == 4
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--baseline-cli", type=Path,
                        default=ROOT / ".cache/tablecraft-bin/ignored-null-int32-cli-20261025")
    parser.add_argument("--originals", type=Path,
                        default=ROOT / ".cache/tablecraft-fixtures/ignore-null-wide-indexes-20261025")
    parser.add_argument("--jar", type=Path,
                        default=ROOT / ".cache/tablecraft-fixtures/jackcess-5.0.4.jar")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists() and not args.log.exists()
    assert digest(args.cli.read_bytes()) == args.cli_sha256
    assert digest(args.baseline_cli.read_bytes()) == BASELINE_SHA256
    assert digest(args.jar.read_bytes()) == JAR_SHA256
    tools = [
        args.cli, args.baseline_cli, args.jar, Path(__file__),
        ROOT / "docs/testing/compare_ignored_null_indexes.py",
        ROOT / "docs/testing/IgnoreNullWideIndexFixture.java",
    ]
    for profile in PROFILES:
        original = args.originals / f"IgnoreWide-{profile}.accdb"
        assert digest(original.read_bytes()) == ORIGINAL_SHA256[profile]
        tools.extend((original, args.originals / f"{profile}-getter.stdout"))
        names = ("IgnoredCurrency", "IgnoredInt64") if profile in ("V2016", "V2019") else ("IgnoredCurrency",)
        tools.extend(args.originals / f"{profile}-{name}-baseline-indexes.tsv" for name in names)
    before = [pin(path) for path in tools]
    selected_sources = source_objects(args.revision)
    native_before = {path: pin(ROOT / path) for path in SOURCE_PATHS}
    for path, source in selected_sources.items():
        assert native_before[path]["sha256"] == source["sha256"]

    replay = Replay(args.log)
    observations = {}
    for profile in PROFILES:
        path = args.originals / f"IgnoreWide-{profile}.accdb"
        original = path.read_bytes()
        tables = getter((args.originals / f"{profile}-getter.stdout").read_text(encoding="utf-8"))
        observations[profile] = {}
        for name, table in tables.items():
            currency = table["type"] == "MONEY"
            assert currency or table["type"] == "BIG_INT"
            assert currency or profile in ("V2016", "V2019")
            observations[profile][name] = {
                "actual_getter_rows": len(table["rows"]), "type": table["type"],
                "indexes": table["metadata"],
                "raw_node_roots": raw_roots(args.originals, profile, name, table, original),
            }
            common = [label(args.cli), "--format", "tsv"]
            baseline = [label(args.baseline_cli), "--format", "tsv"]
            check = ["inspect", label(path), "--table", name, "--check", "--native-steps", "10000000"]
            replay.run(
                [*common, *check],
                contains=("Rows checked\t701", "Indexes checked\t4", "Indexes not checked\t0"),
            )
            replay.run(
                [*baseline, *check],
                contains=("Rows checked\t701", "Indexes checked\t0", "Indexes not checked\t4"),
            )

            def query(predicate, parameters=(), index=None, automatic=False,
                      executable=common, expected=None, refused=False):
                declaration = ""
                if parameters:
                    kind = "CURRENCY" if currency else "BIGINT"
                    declaration = "PARAMETERS " + ",".join(f"{key} {kind}" for key, _ in parameters) + "; "
                sql = f"{declaration}SELECT ID FROM {name}" + (f" WHERE {predicate}" if predicate else "")
                command = [
                    *executable, "query", "run", sql, "--database", label(path),
                    "--number-format", "invariant",
                ]
                for key, value in parameters:
                    command.extend(("--param", parameter(currency, key, value)))
                if index:
                    command.extend(("--index", name, index))
                if automatic:
                    command.append("--auto-index")
                replay.run(command, expected_rows=expected, refused=refused)

            by_id = dict(table["rows"])
            for index, column in INDEX_COLUMNS.items():
                field = "KeyValue" if column == 0 else "UniqueValue"
                point = POINTS[index]
                expected = [
                    identifier for identifier in table["entries"][index]
                    if by_id[identifier][column] == point
                ]
                query(f"{field}=P", (("P", point),), index=index, expected=expected)
                expected = [
                    identifier for identifier in table["entries"][index]
                    if LOWER <= by_id[identifier][column] <= UPPER
                ]
                query(f"{field}>=P AND {field}<=Q", (("P", LOWER), ("Q", UPPER)), index=index, expected=expected)
            query("KeyValue=P", (("P", UPPER),), automatic=True,
                  expected=[identifier for identifier, row in table["rows"] if row[0] == UPPER])
            expected = [
                identifier for identifier, row in table["rows"]
                if row[0] is not None and LOWER <= row[0] <= UPPER
            ]
            query("KeyValue>=P AND KeyValue<=Q", (("P", LOWER), ("Q", UPPER)),
                  automatic=True, expected=expected)
            query("KeyValue IS NULL", automatic=True,
                  expected=[identifier for identifier, row in table["rows"] if row[0] is None])
            query("", index="IgnoredAsc", refused=True)
            query("KeyValue IS NULL", index="IgnoredAsc", refused=True)
            query("KeyValue=P", (("P", UPPER),), index="IgnoredAsc", executable=baseline, refused=True)

    after = [pin(path) for path in tools]
    assert before == after
    native_after = {path: pin(ROOT / path) for path in SOURCE_PATHS}
    assert native_before == native_after
    assert selected_sources == source_objects(args.revision)
    report = {
        "scope": "Independent exact signed/scaled getter model; not native Access qualification",
        "native_source_revision": args.revision,
        "selected_native_source_objects": selected_sources,
        "selected_native_live_before": native_before,
        "selected_native_live_after": native_after,
        "build_scope_limitation": (
            "Frozen executable hash is decisive; selected native objects do not attest "
            "the complete build checkout."
        ),
        "immutable_pins_before": before, "immutable_pins_after": after,
        "observations": observations, "controls": len(replay.controls),
        "disagreements": replay.disagreements, "raw_log": pin(args.log),
        "results": replay.controls,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"controls": len(replay.controls), "disagreements": replay.disagreements}))
    return bool(replay.disagreements)


if __name__ == "__main__":
    raise SystemExit(main())
