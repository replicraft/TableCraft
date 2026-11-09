"""Compare selected native index reads on unchanged 5,000-page ACE originals."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from compare_ignored_null_indexes import ROOT, digest, pin

ORIGINAL_SHA256 = {
    "V2016": "a92022ed6a1a63335cf82901da00d9d9b7d6013792042f90fce328304c0319f4",
    "V2019": "0dc0bf89240ab499427e546502e983547715b3a27698dd4c1eb6a86d96cf558b",
}
ORACLE_SHA256 = {
    "V2016": "09018306f3c8b51cbb3e513504e746b7d2edd0333e02bea30387e29aa25feb33",
    "V2019": "46022af4c5f7719abdb0906ede3212124c1936589e53990ac25703361a8559ea",
}
SOURCE_PATHS = (
    "crates/engines/tablecraft-ace/src/format/allocation.rs",
    "crates/engines/tablecraft-ace/src/format/allocation/cursor.rs",
    "crates/engines/tablecraft-ace/src/format/allocation/cursor/membership.rs",
    "crates/engines/tablecraft-ace/src/table/indexes/read.rs",
    "crates/engines/tablecraft-ace/src/table/indexes/read/membership.rs",
)


def getter_model(path):
    rows = []
    owned = []
    entries = []
    declared = owned_count = None
    for fields in csv.reader(io.StringIO(path.read_text()), delimiter="\t"):
        if fields[0] == "TABLE":
            declared = int(fields[1])
            assert int(fields[3]) == 4096
        elif fields[0] == "PAGE":
            owned.append(int(fields[1]))
        elif fields[0] == "OWNED":
            owned_count = int(fields[1])
        elif fields[0] == "ROW":
            rows.append({
                "id": int(fields[1]), "page": int(fields[2]),
                "slot": int(fields[3]), "payload_sha256": fields[4],
            })
        elif fields[0] == "INDEX":
            assert int(fields[1]) == declared
        elif fields[0] == "ENTRY":
            entries.append(int(fields[1]))
    assert declared == len(rows) == owned_count == len(owned) == len(set(owned)) == 5000
    assert [row["id"] for row in rows] == entries == list(range(5000))
    assert all(row["page"] in owned for row in rows)
    return rows


def cases(rows):
    identifiers = [row["id"] for row in rows]
    required = ["--index", "Pages", "PrimaryKey"]
    for identifier in (identifiers[0], identifiers[len(identifiers) // 2], identifiers[-1], identifiers[-1] + 1):
        expected = [[str(value)] for value in identifiers if value == identifier]
        yield (
            f"required-point-{identifier}", f"SELECT ID FROM Pages WHERE ID={identifier}",
            required, ["ID"], expected, None,
        )
    for name, flags in [("required-range", required), ("automatic-range", ["--auto-index"])]:
        yield (
            name, "SELECT ID FROM Pages WHERE ID>=2500 AND ID<=2503", flags,
            ["ID"], [[str(value)] for value in identifiers if 2500 <= value <= 2503], None,
        )
    yield (
        "automatic-last", f"SELECT ID FROM Pages WHERE ID={identifiers[-1]}",
        ["--auto-index"], ["ID"], [[str(identifiers[-1])]], None,
    )
    yield (
        "sequential-control", "SELECT ID FROM Pages", [],
        ["ID"], [[str(value)] for value in identifiers], None,
    )
    yield (
        "complete-index-ceiling", "SELECT ID FROM Pages", required,
        [], [], "structural limit",
    )
    for identifier in (identifiers[0], identifiers[-1]):
        columns = ["ID", *[f"B{column}" for column in range(12)]]
        yield (
            f"selected-payload-{identifier}",
            "SELECT " + ",".join(columns) + f" FROM Pages WHERE ID={identifier}",
            required, columns, identifier, None,
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "inputs", "report", "log"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--compiled-revision", required=True)
    args = parser.parse_args()
    if args.report.exists() or args.log.exists():
        parser.error("Report and log paths must be fresh")
    if digest(args.cli.read_bytes()) != args.cli_sha256:
        parser.error("Frozen executable checksum mismatch")
    revision = subprocess.check_output(
        ["git", "rev-parse", args.compiled_revision], cwd=ROOT, text=True, timeout=30,
    ).strip()
    sources = {}
    for path in SOURCE_PATHS:
        data = subprocess.check_output(["git", "show", f"{revision}:{path}"], cwd=ROOT, timeout=30)
        sources[path] = {"bytes": len(data), "sha256": digest(data)}
    artifacts = [
        args.cli, Path(__file__), ROOT / "docs/testing/compare_ignored_null_indexes.py",
        ROOT / "docs/testing/LargeUsageMapProbe.java",
    ]
    if digest(artifacts[-1].read_bytes()) != "bb207c72e9324f166a160929e9596da09d4b15b45de7d2414d44ec20eed88b13":
        raise ValueError("Public writer/getter recipe differs from the reviewed original")
    for profile, expected_hash in ORIGINAL_SHA256.items():
        database = args.inputs / f"{profile}.accdb"
        oracle = args.inputs / f"{profile}-oracle.stdout"
        if digest(database.read_bytes()) != expected_hash:
            raise ValueError("Original database differs from the reviewed public-writer fixture")
        if digest(oracle.read_bytes()) != ORACLE_SHA256[profile]:
            raise ValueError("Getter transcript differs from the reviewed baseline")
        artifacts.extend([database, oracle])
    before = [pin(path) for path in artifacts]
    observations = []
    compared_rows = 0
    with args.log.open("x", encoding="utf-8") as log:
        for profile in ORIGINAL_SHA256:
            rows = getter_model(args.inputs / f"{profile}-oracle.stdout")
            by_id = {row["id"]: row for row in rows}
            for name, sql, flags, headings, expected, refusal in cases(rows):
                command = [
                    str(args.cli), "--format", "tsv", "query", "run", sql,
                    "--database", str(args.inputs / f"{profile}.accdb"),
                    "--native-bytes", "67108864", "--native-steps", "20000000",
                    "--execution-bytes", "67108864", *flags,
                ]
                result = subprocess.run(command, capture_output=True, text=True, timeout=60)
                disagreement = None
                try:
                    if refusal:
                        assert result.returncode == 2 and not result.stdout
                        assert refusal in result.stderr
                    else:
                        assert result.returncode == 0 and not result.stderr
                        table = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
                        assert table[0] == headings
                        if isinstance(expected, int):
                            assert len(table) == 2 and len(table[1]) == len(headings)
                            assert table[1][0] == str(expected)
                            payload = []
                            for value in table[1][1:]:
                                assert value.startswith("0x")
                                decoded = bytes.fromhex(value[2:])
                                assert len(decoded) == 240
                                payload.append(decoded)
                            assert hashlib.sha256(b"".join(payload)).hexdigest() == by_id[expected]["payload_sha256"]
                        else:
                            assert table[1:] == expected
                        compared_rows += len(table) - 1
                except (AssertionError, ValueError, IndexError) as error:
                    disagreement = str(error) or "Unexpected exit, output or payload digest"
                observation = {
                    "profile": profile, "case": name, "command": command,
                    "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                    "agrees": disagreement is None, "disagreement": disagreement,
                }
                log.write(json.dumps(observation) + "\n")
                observations.append(observation)
    if [pin(path) for path in artifacts] != before:
        raise ValueError("Executable, fixture, getter or reference changed during replay")
    report = {
        "scope": "Selected qualified index reads over 5,000-owned-page ACE originals; complete-read ceiling preserved",
        "controls": len(observations), "disagreements": sum(not row["agrees"] for row in observations),
        "rows_compared": compared_rows, "compiled_revision": revision,
        "selected_compiled_source_objects": sources,
        "immutable_artifacts_before_after": before, "raw_log": pin(args.log),
        "observations": observations,
        "limitations": [
            "Independent public-writer/getter evidence is not native Microsoft Access qualification.",
            "Complete indexed reads/checks and mutation materialization retain their existing map ceilings.",
            "Selected source objects are not a complete compiler-input manifest.",
        ],
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "disagreements": report["disagreements"]}))
    raise SystemExit(bool(report["disagreements"]))


if __name__ == "__main__":
    main()
