"""Compare explicit cached Text/Memo flag families with independent inert getters.

Only authored Memo Hyperlink flags broaden admission. Stored values stay raw and
possibly stale; no formula, hyperlink navigation or RichText conversion executes.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from cli_tsv import display_tsv
from compare_browsing import display
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PROFILES = ("V2016", "V2019")


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=120
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def records(output):
    tables = {}
    for line in output.splitlines():
        tag, name, *cells = line.split("\t")
        if tag == "D":
            carrier, length, calculated, compressed, hyperlink, result, expression, text_format = cells
            assert calculated == "true" and compressed == "false"
            assert bytes.fromhex(expression[1:]).decode() == '"recomputed"'
            assert (hyperlink == "true") == ("Hyperlink" in name)
            assert text_format == ("1" if "RichText" in name else "N")
            assert (carrier, int(length), int(result)) == (
                ("TEXT", 423, 10) if name.removeprefix("Empty").startswith("Short") else ("MEMO", 0, 12)
            )
            tables[name] = {"carrier": carrier, "length": int(length), "hyperlink": hyperlink == "true",
                            "text_format": text_format, "rows": []}
        elif tag == "R":
            ordinal, identity, cached = cells
            assert int(ordinal) == len(tables[name]["rows"])
            tables[name]["rows"].append((int(identity), cached))
        else:
            raise AssertionError("Unexpected original cached Text oracle record")
    return tables


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    oracle = RECIPES / "CalculatedTextFamilyOracle.java"
    paths = {
        args.cli, args.jar, oracle, Path(__file__), RECIPES / "CalculatedTextFamilyFixture.java",
        *[RECIPES / name for name in (
            "cli_tsv.py", "compare_browsing.py", "compare_calculated_decimals.py", "float_model.py",
        )],
        *sorted((ROOT / "crates/engines/tablecraft-ace/src/table/calculated").glob("*.rs")),
        ROOT / "crates/backends/tablecraft-access/src/source/schema.rs",
        ROOT / "crates/backends/tablecraft-access/src/source/projection.rs",
        ROOT / "crates/backends/tablecraft-access/src/source/snapshot.rs",
        *[args.directory / f"{profile}.accdb" for profile in PROFILES],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    cached_cell_comparisons = 0
    observations = []

    def cli(*arguments):
        return display_tsv(execute(args.cli, "--format", "tsv", *arguments))

    def query(path, sql, *extra):
        return cli("query", "run", sql, "--database", path, "--stored-calculations", *extra)

    def refused(*arguments):
        result = subprocess.run(
            list(map(str, (args.cli, *arguments))), capture_output=True, timeout=120
        )
        assert result.returncode == 2 and not result.stdout, result.stderr

    for profile in PROFILES:
        path = args.directory / f"{profile}.accdb"
        original = path.read_bytes()
        output = execute("java", "-cp", args.jar, oracle, path)
        tables = records(output)
        assert len(tables) == 16
        controls += len(tables)
        for name, metadata in tables.items():
            inspected = cli("inspect", path, "--table", name)
            columns = {
                cells[0]: cells[1:] for cells in (line.split("\t") for line in inspected.splitlines())
                if len(cells) == 6
            }
            flags = "0x82" if metadata["hyperlink"] else "0x02"
            kind = "10" if metadata["carrier"] == "TEXT" else "12"
            assert columns["Cached"] == [kind, str(metadata["length"]), flags, "0xc0", "Unsupported"]
            controls += 1
            header = f"Table\t{name}\nColumn\tCached\nValues\tStored; not recalculated\n"
            header += "Row ordinal\tStored Text value\n"
            expected_inspect = header + "".join(
                f"{ordinal}\t{display(value)}\n" for ordinal, (_, value) in enumerate(metadata["rows"])
            )
            assert cli("inspect", path, "--table", name, "--stored-calculation", "Cached") == expected_inspect
            expected_query = "ID\tCached\n" + "".join(
                f"{identity}\t{display(value)}\n" for identity, value in metadata["rows"]
            )
            assert query(path, f"SELECT ID,Cached FROM [{name}]") == expected_query
            expected_rows = expected_query.replace("ID\tCached\n", "ID\tCached [cached]\n", 1)
            assert cli("rows", path, name, "--stored-calculations") == expected_rows
            controls += 3
            cached_cell_comparisons += 3 * len(metadata["rows"])
            refused("query", "run", f"SELECT Cached FROM [{name}]", "--database", path)
            controls += 1
            if not metadata["rows"]:
                continue

            lengths = [
                "NULL" if value == "N" else str(len(bytes.fromhex(value[1:]).decode().encode("utf-16le")) // 2)
                for _, value in metadata["rows"]
            ]
            assert query(path, f"SELECT ID,Len(Cached) AS Units FROM [{name}]") == "ID\tUnits\n" + "".join(
                f"{identity}\t{length}\n" for (identity, _), length in zip(metadata["rows"], lengths)
            )
            nonnull = sum(value != "N" for _, value in metadata["rows"])
            assert query(path, f"SELECT Count(Cached) AS N FROM [{name}]") == f"N\n{nonnull}\n"
            assert query(path, f"SELECT ID FROM [{name}] WHERE Cached IS NULL") == "ID\n" + "".join(
                f"{identity}\n" for identity, value in metadata["rows"] if value == "N"
            )
            assert cli("query", "run", f"SELECT Count(*) AS N FROM [{name}]", "--database", path) == "N\n8\n"
            controls += 4
            for options in (("--native-bytes", "1"), ("--execution-steps", "1"), ("--output-bytes", "1")):
                refused("query", "run", f"SELECT Cached FROM [{name}]", "--database", path,
                        "--stored-calculations", *options)
                controls += 1

            # Each table authors a unique late cache; corrupt its wrapper only on
            # a disposable copy. Both inspectors and full cached queries refuse it.
            late = ("late-" + name).encode("utf-16le")
            wrapped = bytes(16) + len(late).to_bytes(4, "little") + late + bytes(3)
            assert original.count(wrapped) == 1
            changed = bytearray(original)
            changed[changed.index(wrapped)] = 1
            with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-text-") as directory:
                copy = Path(directory) / "invalid.accdb"
                copy.write_bytes(changed)
                refused("inspect", copy, "--table", name, "--stored-calculation", "Cached")
                refused("rows", copy, name, "--stored-calculations")
                for sql in (
                    f"SELECT TOP 1 Cached FROM [{name}]",
                    f"SELECT Cached FROM [{name}] WHERE ID=-1",
                ):
                    refused("query", "run", sql, "--database", copy, "--stored-calculations")
                assert query(copy, f"SELECT Count(*) AS N FROM [{name}]") == "N\n8\n"
                assert copy.read_bytes() == changed
                controls += 5
            observations.append({
                "profile": profile, "table": name, "carrier": metadata["carrier"],
                "flags": flags, "extended_flags": "0xc0", "text_format": metadata["text_format"],
                "rows": len(metadata["rows"]), "oracle_sha256": hashlib.sha256(output.encode()).hexdigest(),
            })

    assert all(digest(ROOT / path) == value for path, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "cached_cell_comparisons": cached_cell_comparisons, "hashes_unchanged": True,
        "inputs": hashes, "observations": observations,
    }
    with args.report.open("x") as destination:
        destination.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} calculated Text-family controls and {cached_cell_comparisons} cached cell comparisons")


if __name__ == "__main__":
    main()
