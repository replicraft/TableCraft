"""Replay ordinary visible-Memo flag families against independent stored getters.

Hyperlink and RichText remain raw data; no parsing, provider activation, native
Access SQL, freshness, mutation or compressed-child qualification is implied.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from cli_tsv import display_tsv
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PROFILES = ("V2007", "V2016", "V2019")
FAMILIES = ("plain", "hyperlink", "richtext")


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=240
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    paths = {
        args.cli, args.jar, Path(__file__),
        *[RECIPES / name for name in (
            "MemoHistoryFamilyFixture.java", "MemoHistoryFamilyOracle.java",
            "VersionHistoryQueryOracle.java", "VersionHistoryOracle.java",
            "compare_version_history_queries.py", "compare_version_history.py",
            "compare_browsing.py", "compare_cached_pages.py", "compare_composite_constraints.py",
            "compare_complex_links.py", "compare_attachment_columns.py", "collection_cli.py",
            "cli_tsv.py", "compare_calculated_decimals.py", "float_model.py",
        )],
        ROOT / "crates/engines/tablecraft-ace/src/table/complex/history/schema.rs",
        ROOT / "crates/engines/tablecraft-ace/src/table/complex/history/discovery.rs",
        ROOT / "crates/engines/tablecraft-ace/src/table/complex/history/read.rs",
        ROOT / "crates/backends/tablecraft-access/src/source/collections.rs",
        ROOT / "crates/backends/tablecraft-access/src/source/collections/load.rs",
        *[args.directory / family / f"{profile}.accdb" for family in FAMILIES for profile in PROFILES],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    metadata = []
    controls = 0
    replays = []
    for family in FAMILIES:
        directory = args.directory / family
        for profile in PROFILES:
            path = directory / f"{profile}.accdb"
            oracle = execute(
                "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                RECIPES / "MemoHistoryFamilyOracle.java", path,
            )
            for line in oracle.splitlines():
                (
                    tag, table, memo, compressed, hyperlink, text_format,
                    hidden, value, child_compressed, modified, flat,
                ) = line.split("\t")
                assert tag == "D" and child_compressed == "false" and value == memo
                assert (compressed == "true") == (table == "OtherHistories")
                assert (hyperlink == "true") == (family == "hyperlink")
                assert text_format == ("1" if family == "richtext" else "N")
                inspected = display_tsv(execute(
                    args.cli, "inspect", path, "--table", table, "--format", "tsv"
                ))
                fields = {
                    cells[0]: cells[1:] for cells in (row.split("\t") for row in inspected.splitlines())
                    if len(cells) == 6
                }
                flags = "0x82" if family == "hyperlink" else "0x02"
                extended = "0x01" if compressed == "true" else "0x00"
                assert fields[memo] == ["12", "0", flags, extended, "Text"]
                assert fields[hidden] == ["18", "4", "0x07", "0x20", "Unsupported"]
                child = display_tsv(execute(
                    args.cli, "inspect", path, "--table", flat, "--format", "tsv"
                ))
                fields = {
                    cells[0]: cells[1:] for cells in (row.split("\t") for row in child.splitlines())
                    if len(cells) == 6
                }
                assert fields[value] == ["12", "0", "0x02", "0x00", "Text"]
                features = display_tsv(execute(
                    args.cli, "inspect", path, "--table", table, "--features", "--format", "tsv"
                ))
                feature = next(row.split("\t") for row in features.splitlines() if row.startswith(memo + "\t"))
                assert feature[5] == ("TRUE" if family == "hyperlink" else "FALSE")
                assert feature[6] == "TRUE"
                assert feature[7] == ("1" if family == "richtext" else "UNKNOWN")
                metadata.append({
                    "family": family, "profile": profile, "table": table, "memo": memo,
                    "hidden": hidden, "flags": flags, "extended_flags": extended,
                    "text_format": text_format, "value_member": value,
                    "modified_member": modified, "flat_table": flat,
                    "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(),
                })
                controls += 1

        for collation in ("ordinal", "database"):
            output = execute(
                "python3", RECIPES / "compare_version_history_queries.py",
                "--cli", args.cli, "--jar", args.jar,
                "--ace2007", directory / "V2007.accdb",
                "--ace2016", directory / "V2016.accdb",
                "--ace2019", directory / "V2019.accdb",
                "--collation", collation,
            )
            replay = json.loads(output)
            assert replay["query_controls"] == 96 and replay["collation"] == collation
            assert replay["cli_sha256"] == hashes[str(args.cli.resolve().relative_to(ROOT))]
            replays.append({"family": family, **replay})
            controls += replay["query_controls"]

    assert all(digest(ROOT / path) == value for path, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "hashes_unchanged": True, "inputs": hashes, "metadata": metadata, "replays": replays,
    }
    with args.report.open("x") as destination:
        destination.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} ordinary visible-Memo/history family controls; all hashes unchanged")


if __name__ == "__main__":
    main()
