"""Compare original Boolean RI getters, exact memberships and corrupt-copy refusals.

This is an offline read-audit model, not native Access write/SQL qualification.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from cli_tsv import display_tsv
from compare_boolean_indexes import canonical
from compare_browsing import display
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PAGE_BYTES = 4096
PROFILES = ("V2016", "V2019")


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=120
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def records(output):
    tables = {}
    relationships = {}
    indexes = {}
    for line in output.splitlines():
        tag, *cells = line.split("\t")
        if tag == "T":
            tables[cells[0]] = {"columns": cells[1].split(","), "rows": [], "fields": {}}
        elif tag == "C":
            table, column, kind, required = cells
            tables[table]["fields"][column] = (kind, required)
        elif tag == "I":
            indexes[(cells[0], cells[1])] = cells[2:]
        elif tag == "R":
            table, ordinal, page, slot, *values = cells
            assert int(ordinal) == len(tables[table]["rows"])
            tables[table]["rows"].append((int(page), int(slot), values))
        elif tag == "L":
            name, parent, child, ordinal, source, target, enforced, unique, flags = cells
            assert enforced == "true"
            relationship = relationships.setdefault(name, {
                "parent": parent, "child": child, "pairs": [],
                "one_to_one": unique == "true", "flags": int(flags),
            })
            assert int(ordinal) == len(relationship["pairs"])
            relationship["pairs"].append((source, target))
        else:
            raise AssertionError("Unexpected original oracle record")
    return tables, relationships, indexes


def key_values(table, columns):
    positions = [table["columns"].index(column) for column in columns]
    return [tuple(values[position] for position in positions) for _, _, values in table["rows"]]


def slot_bounds(original, row):
    page, slot, _ = row
    start = page * PAGE_BYTES
    raw = int.from_bytes(original[start + 14 + slot * 2:start + 16 + slot * 2], "little")
    assert raw & 0xe000 == 0, "Mutation controls require original inline live roots"
    end = PAGE_BYTES if slot == 0 else int.from_bytes(
        original[start + 12 + slot * 2:start + 14 + slot * 2], "little"
    ) & 0x1fff
    return start + (raw & 0x1fff), start + end


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    oracle = RECIPES / "BooleanRelationshipOracle.java"
    paths = {
        args.cli, args.jar, oracle, Path(__file__),
        RECIPES / "BooleanRelationshipFixture.java", RECIPES / "IndexRowsOracle.java",
        *[RECIPES / name for name in (
            "cli_tsv.py", "compare_boolean_indexes.py", "compare_browsing.py",
            "compare_calculated_decimals.py", "compare_composite_constraints.py", "float_model.py",
        )],
        *sorted((ROOT / "crates/engines/tablecraft-ace/src/constraints/relationships").glob("*.rs")),
        *sorted((ROOT / "crates/engines/tablecraft-ace/src/constraints").glob("*.rs")),
        ROOT / "crates/engines/tablecraft-ace/src/table/mutation/gates.rs",
    }
    for profile in PROFILES:
        paths.add(args.directory / f"{profile}.accdb")
        paths.add(args.directory / f"null-orphan-{profile}.accdb")
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    compared_cells = 0
    observations = []

    def cli(*arguments):
        return display_tsv(execute(args.cli, "--format", "tsv", *arguments))

    def refused(path, table, message):
        result = subprocess.run(
            [str(args.cli), "inspect", str(path), "--table", table, "--constraints"],
            capture_output=True, text=True, timeout=120,
        )
        assert result.returncode == 2 and not result.stdout and message in result.stderr, result.stderr

    for profile in PROFILES:
        path = args.directory / f"{profile}.accdb"
        original = path.read_bytes()
        output = execute("java", "-cp", args.jar, oracle, path)
        tables, relationships, indexes = records(output)
        assert len(relationships) == 6
        assert relationships["UniqueBoolKeys"]["one_to_one"]
        assert relationships["UniqueMixedKeys"]["one_to_one"]
        assert indexes[("BoolParents", "BoolKey")][:5] == ["true", "true", "false", "true", "false"]
        assert indexes[("MixedParents", "MixedKey")] == [
            "false", "true", "false", "false", "false", "Flag:D,Part:A,Other:D",
        ]

        for table in tables.values():
            for column, (kind, _) in table["fields"].items():
                if kind == "BOOLEAN":
                    position = table["columns"].index(column)
                    assert all(row[2][position] in ("B0", "B1") for row in table["rows"])
        for relationship in relationships.values():
            parent = tables[relationship["parent"]]
            child = tables[relationship["child"]]
            sources, targets = zip(*relationship["pairs"])
            parent_keys = key_values(parent, sources)
            child_keys = key_values(child, targets)
            assert len(set(parent_keys)) == len(parent_keys)
            assert all(key in parent_keys for key in child_keys)
            if relationship["one_to_one"]:
                assert len(set(child_keys)) == len(child_keys)
        controls += len(relationships)

        for name, table in tables.items():
            expected = "\t".join(table["columns"]) + "\n"
            expected += "".join(
                "\t".join(display(value) for value in values) + "\n"
                for _, _, values in table["rows"]
            )
            assert cli("query", "run", f"SELECT * FROM [{name}]", "--database", path) == expected
            compared_cells += len(table["columns"]) * len(table["rows"])
            audit = cli("inspect", path, "--table", name, "--constraints")
            assert f"Rows checked\t{len(table['rows'])}\n" in audit
            assert "unsupported" not in audit.lower()
            for relation, metadata in relationships.items():
                if name in (metadata["parent"], metadata["child"]):
                    assert f"{relation}\tChecked: exact key membership\n" in audit
            controls += 2

        node_roots = []
        for name, index in (
            ("BoolChildren", "BoolKeys"),
            ("TrueChildren", "TrueKeys"),
            ("MixedParents", "MixedKey"),
            ("MixedChildren", "MixedKeys"),
        ):
            table = tables[name]
            metadata = cli("inspect", path, "--table", name, "--indexes")
            roots = {
                int(line.split("\t")[3]) for line in metadata.splitlines()
                if line.startswith(index + "\t")
            }
            assert len(roots) == 1
            root = roots.pop()
            assert original[root * PAGE_BYTES] == 3, "Expected original node/leaf tree"
            node_roots.append({"table": name, "index": index, "root": root})
            index_rows = execute(
                "java", "-cp", args.jar, RECIPES / "IndexRowsOracle.java",
                path, name, index, *table["columns"],
            )
            expected = "\t".join(table["columns"]) + "\n" + canonical(index_rows, table["columns"])
            assert cli(
                "query", "run", f"SELECT * FROM [{name}]", "--database", path,
                "--index", name, index,
            ) == expected
            compared_cells += len(table["columns"]) * len(table["rows"])
            controls += 1

        cases = []
        for name, ordinal, mask, message in (
            ("TrueChildren", -1, 2, "foreign key has no referenced record"),
            ("MixedChildren", 1499, 2, "foreign key has no referenced record"),
            ("BoolParents", -1, 2, "referenced key is not unique"),
            ("BoolUniqueChildren", -1, 2, "one-to-one relationship has duplicate dependent keys"),
            ("MixedParents", -1, 8, "referenced key is not unique"),
            ("MixedUniqueChildren", 1, 10, "one-to-one relationship has duplicate dependent keys"),
        ):
            changed = bytearray(original)
            _, end = slot_bounds(original, tables[name]["rows"][ordinal])
            changed[end - 1] ^= mask
            cases.append((name, changed, message))
        changed = bytearray(original)
        start, end = slot_bounds(original, tables["MixedChildren"]["rows"][-1])
        changed[start + 6:start + 10] = (-1).to_bytes(4, "little", signed=True)
        changed[end - 1] |= 4
        cases.append(("MixedChildren", changed, "foreign key has no referenced record"))

        with tempfile.TemporaryDirectory(prefix="tablecraft-boolean-ri-") as directory:
            copy = Path(directory) / "invalid.accdb"
            for table, changed, message in cases:
                copy.write_bytes(changed)
                refused(copy, table, message)
                assert copy.read_bytes() == changed
                controls += 1

        invalid = args.directory / f"null-orphan-{profile}.accdb"
        invalid_output = execute("java", "-cp", args.jar, oracle, invalid)
        invalid_tables, invalid_relationships, _ = records(invalid_output)
        assert invalid_tables["EmptyParents"]["rows"] == []
        assert invalid_tables["EmptyChildren"]["rows"][0][2] == ["I32:3", "B0"]
        assert invalid_relationships["EmptyKeys"]["pairs"] == [("Flag", "Flag")]
        refused(invalid, "EmptyChildren", "foreign key has no referenced record")
        assert cli("query", "run", "SELECT * FROM EmptyChildren", "--database", invalid) == "ID\tFlag\n3\tFALSE\n"
        controls += 2
        observations.append({
            "profile": profile, "tables": len(tables), "relationships": len(relationships),
            "rows": sum(len(table["rows"]) for table in tables.values()),
            "node_roots": node_roots, "corrupt_copies": len(cases),
            "oracle_sha256": hashlib.sha256(output.encode()).hexdigest(),
            "invalid_oracle_sha256": hashlib.sha256(invalid_output.encode()).hexdigest(),
        })

    assert all(digest(ROOT / path) == value for path, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "compared_cells": compared_cells, "hashes_unchanged": True,
        "inputs": hashes, "observations": observations,
    }
    with args.report.open("x") as destination:
        destination.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} Boolean relationship controls and {compared_cells} getter/index cells")


if __name__ == "__main__":
    main()
