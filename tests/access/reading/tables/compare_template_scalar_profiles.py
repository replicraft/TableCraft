"""Compare seven original inert audit templates with independent XML/ZIP cells.

The files are original repository-owned synthetic XML packages. They are not
Access-created templates, and the historical authoring steps are not claimed to
be a byte-reproducible generator. Only TableCraft executes SQL.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[2]
COMPILED_REVISION = "1286af1ea0ea39c0d350c4d64bb235fb8132f196"
CLI_SHA256 = "398b9c4f4b9e41521c2fd70750b5f946d5fcfa38133f3f66304f11c92ea91808"
OBJECT = "http://schemas.microsoft.com/office/access/2005/04/template/object-metadata"
INPUTS = {
    "typed-utf8": "95adcfbebc6e75ac46a83b3be4a39082cd64c1c2a504e93c4038b79f321ce55d",
    "typed-utf16": "871d72e6bb2dc40518c6aa6491d2d24546d48f808a6ce2eb85dbb852e915937c",
    "typed-local-named-type": "3a9759e2565159852fdd59ee6767bbe2e0b12f30d3f10dc2009db3cbbfe55e27",
    "list-note": "2ad73278023bed67c7471a806b58ed5184d0982606f2b4acab11a47308e1e344",
    "list-single-choice": "76de18416ff4297bf7b3ef11f4bb005195ddd54114eac0a9360f9fd277299f90",
    "list-single-choice-valid-default": "4e56add6c11ece32b61b70d78471107c00b43dd40270346d28029d43068dc67c",
    "unicode-ncname": "cd6349cafd5a1c670507ed2604671f715a8e4a0542b36fb7bee8cc16bf179bd6",
}
INTEGER_FIELDS = {"ID", "Tiny", "Large", "编号"}
AUTHORING_BASES = [
    "docs/testing/compare_template_packages.py",
    "docs/testing/compare_template_tables.py",
    "docs/testing/compare_template_lists.py",
]
CLI_PATHS = [
    "apps/tablecraft-cli/src/output.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/args.rs",
]


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def relative(path):
    return str(Path(path).resolve().relative_to(ROOT))


def invoke(arguments):
    return subprocess.run(
        list(map(str, arguments)), cwd=ROOT, capture_output=True,
        check=False, timeout=120,
    )


def compiled_sources():
    prefix = "crates/backends/tablecraft-access/src/template"
    listing = invoke(["git", "ls-tree", "-r", "--name-only", COMPILED_REVISION, prefix])
    if listing.returncode:
        raise RuntimeError(listing.stderr.decode())
    paths = listing.stdout.decode().splitlines()
    paths = sorted({path for path in paths if path.endswith(".rs")} | set(CLI_PATHS))
    records = {}
    for path in paths:
        result = invoke(["git", "show", f"{COMPILED_REVISION}:{path}"])
        if result.returncode:
            raise RuntimeError(result.stderr.decode())
        records[path] = hashlib.sha256(result.stdout).hexdigest()
    return records


def display(value):
    if isinstance(value, str):
        # These fixtures contain ordinary Unicode text without control characters.
        # JSON quotes/escapes agree with the existing scalar display for this set.
        if any(ord(character) < 32 for character in value):
            raise ValueError("This model does not admit control-character text")
        return json.dumps(value, ensure_ascii=False)
    return str(value)


def model(path, fields):
    with zipfile.ZipFile(path) as archive:
        data_bytes = archive.read("data.xml")
        metadata = ET.fromstring(archive.read("metadata.xml"))
        data = ET.fromstring(data_bytes)
    table = metadata.find(f"{{{OBJECT}}}Name").text
    instances = data.find("dataroot")
    rows = []
    for instance in instances:
        row = []
        for field in fields:
            raw_name = field.replace(" ", "_x0020_")
            cell = instance.find(raw_name)
            if cell is None:
                raise ValueError(f"Missing required modeled cell: {field}")
            text = cell.text if cell.text is not None else ""
            row.append(int(text) if field in INTEGER_FIELDS else text)
        rows.append(row)
    rows.sort(key=lambda row: row[0])
    return table, rows, hashlib.sha256(data_bytes).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Refuse to overwrite historical evidence")
    if digest(args.cli) != CLI_SHA256:
        raise RuntimeError("Frozen compiled CLI differs")

    inputs = {name: args.directory / f"{name}.accdt" for name in INPUTS}
    for name, path in inputs.items():
        if digest(path) != INPUTS[name]:
            raise RuntimeError(f"Original audit input differs: {name}")
    paths = [args.cli, Path(__file__), *inputs.values()]
    paths.extend(ROOT / path for path in AUTHORING_BASES)
    before = {relative(path): digest(path) for path in paths}
    compiled = compiled_sources()
    controls = []
    compared_rows = 0
    compared_cells = 0
    for name, database in inputs.items():
        if name.startswith("typed-"):
            fields = ["ID", "Full Name", "Tiny", "Large"]
        elif name.startswith("list-"):
            fields = ["ID", "Notes", "Full Name"]
        else:
            fields = ["编号"]
        table, rows, raw_data_sha256 = model(database, fields)
        columns = ",".join(f"[{field}]" for field in fields)
        sql = f"SELECT {columns} FROM [{table}] ORDER BY [{fields[0]}]"
        command = [
            args.cli, "--format", "tsv", "query", "run", sql,
            "--database", database,
        ]
        result = invoke(command)
        stdout = result.stdout.decode("utf-8")
        actual = list(csv.reader(io.StringIO(stdout), delimiter="\t", strict=True))
        expected = [fields, *[[display(cell) for cell in row] for row in rows]]
        agrees = result.returncode == 0 and actual == expected and not result.stderr
        controls.append({
            "case": name, "input": relative(database), "sql": sql,
            "exit_code": result.returncode, "stdout": stdout,
            "stderr": result.stderr.decode("utf-8"), "expected": expected,
            "model_rows": rows, "data_part_sha256": raw_data_sha256,
            "agrees": agrees,
        })
        compared_rows += len(rows)
        compared_cells += sum(map(len, rows))

    after = {relative(path): digest(path) for path in paths}
    compiled_after = compiled_sources()
    report = {
        "schema_version": 1, "date": "2026-10-25",
        "compiled_revision": COMPILED_REVISION,
        "compiled_source_sha256": compiled,
        "compiled_sources_unchanged": compiled == compiled_after,
        "immutable_sha256_before": before, "immutable_sha256_after": after,
        "originals_and_tools_unchanged": before == after,
        "controls": controls, "compared_rows": compared_rows,
        "compared_cells": compared_cells,
        "successful_controls": sum(control["agrees"] for control in controls),
        "total_controls": len(controls),
        "accepted": all(control["agrees"] for control in controls)
            and before == after and compiled == compiled_after,
        "scope": "Independent stdlib XML/ZIP stored-cell model; no reference SQL or native Access qualification",
        "provenance": "Original repository-owned synthetic audit packages based on the pinned template helpers; no byte-reproducible historical authoring claim",
        "exclusions": [
            "Earlier frozen audit CLI refused UTF16, local named types and Choice; historical probe reports are preserved separately.",
            "The existing Chinese-name input already worked before this replay; it is a regression control, not a newly fixed NCName case.",
            "This frozen CLI predates the exact XML lexical-name correction; no new lexical-boundary CLI qualification is claimed.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({
        "accepted": report["accepted"], "controls": len(controls),
        "successful": report["successful_controls"], "rows": compared_rows,
        "cells": compared_cells, "immutable_pins": len(before),
        "compiled_source_pins": len(compiled),
    }))
    if not report["accepted"]:
        raise SystemExit("Disagreements preserved in fresh report")


if __name__ == "__main__":
    main()
