"""Independent getter-backed external named-source namespace replay.

Reference SQL is inspected only. Native files and saved fragments are original;
lexical ownership expectations are TableCraft engineering contracts.
"""

import argparse
from collections import Counter
import csv
import hashlib
from io import StringIO
import itertools
import json
from pathlib import Path
import subprocess

from compare_saved_external import decode, display


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019", "V2000", "V2007")
NODES = ("root", "one", "two")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def getters(path):
    return [[decode(value) for value in line.split("\t")] for line in path.read_text().splitlines()]


def pivot(rows, fixed):
    headings = [1, 2] if fixed else sorted({row[2] for row in rows})
    cells = []
    for heading in headings:
        scores = [row[1] for row in rows if row[2] == heading]
        cells.append(str(sum(scores)) if scores else "NULL")
    return [["G", *map(str, headings)], ["7", *cells]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--source-manifest", type=Path, required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.source_manifest) == args.manifest_sha256
    paths = {args.cli, args.jar, args.source_manifest, Path(__file__), ROOT / "Cargo.lock"}
    paths.update(ROOT / "docs/testing" / name for name in (
        "ExternalNamedFixture.java", "ExternalNamedOracle.java", "compare_saved_external.py",
    ))
    paths.update(args.directory.rglob("*"))
    for line in args.source_manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        paths.add(path)
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths) if path.is_file()}
    observations = []
    modeled_cells = 0

    def check(label, command, expected=None, refusal=None, contains=()):
        nonlocal modeled_cells
        command = list(map(str, command))
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        if refusal:
            assert result.returncode != 0 and not result.stdout, (label, result.stdout, result.stderr)
            assert refusal in result.stderr, (label, refusal, result.stderr)
        else:
            assert result.returncode == 0, (label, result.stderr)
            if expected is not None:
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual == expected, (label, actual, expected)
                modeled_cells += sum(len(row) for row in expected[1:])
            for text in contains:
                assert text in result.stdout, (label, text, result.stdout)
        observations.append({"label": label, "command": command, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr, "expected_refusal": refusal})
        return result.stdout

    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        database = {node: args.directory / profile / (node + suffix) for node in NODES}
        rows = {}
        prefix = [args.cli, "--format", "tsv"]
        for node, path in database.items():
            records = getters(path.with_suffix(suffix + ".getters.tsv"))
            columns = [record[2:] for record in records if record[:2] == ["COLUMN", "Items"]]
            assert columns == [["ID", "LONG"], ["Score", "LONG"], ["Quarter", "LONG"], ["Formula", "TEXT"]]
            rows[node] = [record[2:] for record in records if record[:2] == ["CELL", "Items"]]
            assert len(rows[node]) == NODES.index(node) + 2
            queries = {record[1]: record for record in records if record[0] == "QUERY"}
            assert len(queries) == 16
            for name, query in sorted(queries.items()):
                raw = [record[2:] for record in records if record[0] == "RAW" and record[1] == query[2]]
                output = check(f"{profile}:{node}:inspect:{name}", prefix + ["inspect", path, "--query", name])
                actual = list(csv.reader(StringIO(output), delimiter="\t", strict=True))
                assert actual[:4] == [
                    ["Query", name], ["Native ID", str(query[2])],
                    ["Flags", f"0x{query[3]:08x}"], ["Fragment rows", str(len(raw))],
                ]
                assert Counter(map(tuple, actual[5:])) == Counter(tuple(map(display, record)) for record in raw)
                check(f"{profile}:{node}:reconstruct:{name}",
                      prefix + ["query", "saved", "--database", path, name], contains=("SELECT",))

        run = prefix + ["query", "run", "--database", database["root"],
                        "--number-format", "invariant", "--collation", "ordinal"]
        mappings = ["--external", "one", database["one"], "--external", "two", database["two"],
                    "--external", "third-token", database["two"]]
        mapped = run + mappings
        for node in NODES:
            qualifier = "" if node == "root" else " IN '" + node + "'"
            data = rows[node]
            simple = [["ID", "Score"], *[[str(row[0]), str(row[1])] for row in data]]
            for name in ("Same", "Nested"):
                check(f"{profile}:{node}:named:{name}",
                      mapped + ["SELECT ID,Score FROM " + name + qualifier + " ORDER BY ID"], simple)
            owned = [["ID", "LocalCount", "BaseCount", "NamedCount", "Evaluated"]]
            for identifier, _, _, formula in data:
                left, right = formula.split("+")
                owned.append([str(identifier), *[str(len(data))] * 3, str(int(left) + int(right))])
            check(f"{profile}:{node}:owned", mapped + ["SELECT * FROM Owned" + qualifier + " ORDER BY ID"], owned)
            duplicated = sorted([[str(row[0]), str(row[1])] for row in data] * 2, key=lambda row: int(row[0]))
            check(f"{profile}:{node}:union", mapped + ["SELECT ID,Score FROM UnionRows" + qualifier + " ORDER BY ID"],
                  [["ID", "Score"], *duplicated])
            for name, fixed in (("FixedCross", True), ("DynamicCross", False)):
                check(f"{profile}:{node}:pivot:{name}",
                      mapped + ["SELECT * FROM " + name + qualifier], pivot(data, fixed))
            check(f"{profile}:{node}:parameters",
                  mapped + ["--param", "Floor=long:2", "SELECT ID,Shifted FROM Param" + qualifier + " ORDER BY ID"],
                  [["ID", "Shifted"], *[[str(row[0]), str(row[1] + 2)] for row in data]])
            redirected = [["ID", "Score"], *[[str(row[0]), str(row[1])] for row in rows["two"]]]
            check(f"{profile}:{node}:redirect",
                  mapped + ["SELECT * FROM Redirect" + qualifier + " ORDER BY ID"], redirected)

        check(profile + ":saved-root", mapped + ["--saved", "Nested"],
              [["ID", "Score"], *[[str(row[0]), str(row[1])] for row in rows["root"]]])

        triples = list(itertools.product(rows["root"], rows["one"], rows["two"]))
        check(profile + ":three-owner-caches", mapped + [
            "SELECT R.ID AS R,A.ID AS A,B.ID AS B FROM Same R,"
            "(SELECT ID FROM Nested IN 'one') A,(SELECT ID FROM Nested IN 'two') B ORDER BY R.ID,A.ID,B.ID"
        ], [["R", "A", "B"], *[[str(root[0]), str(one[0]), str(two[0])] for root, one, two in triples]])
        check(profile + ":three-owner-parameters", mapped + [
            "--param", "Floor=long:2",
            "SELECT R.Shifted AS R,A.Shifted AS A,B.Shifted AS B FROM Param R,"
            "(SELECT Shifted FROM Param IN 'one') A,(SELECT Shifted FROM Param IN 'two') B "
            "ORDER BY R.Shifted,A.Shifted,B.Shifted"
        ], [["R", "A", "B"], *[[str(root[1] + 2), str(one[1] + 2), str(two[1] + 2)]
                                  for root, one, two in triples]])
        check(profile + ":caller-subquery-root", mapped + [
            "SELECT ID,(SELECT Count(*) FROM Items) AS N FROM Same IN 'one' ORDER BY ID"
        ], [["ID", "N"], *[[str(row[0]), str(len(rows["root"]))] for row in rows["one"]]])
        check(profile + ":root-domain-sibling", mapped + [
            "SELECT A.BaseCount,DCount('*','Items') AS RootCount "
            "FROM (SELECT BaseCount FROM Owned IN 'one') A"
        ], [["BaseCount", "RootCount"], *[[str(len(rows["one"])), str(len(rows["root"]))] for _ in rows["one"]]])
        fixed = [pivot(rows[node], True)[1][1:] for node in NODES]
        check(profile + ":fixed-three-owner", mapped + [
            "SELECT R.[1] AS R1,A.[1] AS A1,B.[2] AS B2 FROM FixedCross R,"
            "(SELECT * FROM FixedCross IN 'one') A,(SELECT * FROM FixedCross IN 'two') B"
        ], [["R1", "A1", "B2"], [fixed[0][0], fixed[1][0], fixed[2][1]]])
        dynamic = [pivot(rows[node], False) for node in NODES]
        check(profile + ":dynamic-three-owner", mapped + [
            "SELECT R.[9] AS R9,A.[1] AS A1,B.[3] AS B3 FROM DynamicCross R,"
            "(SELECT * FROM DynamicCross IN 'one') A,(SELECT * FROM DynamicCross IN 'two') B"
        ], [["R9", "A1", "B3"], [dynamic[0][1][1], dynamic[1][1][1], dynamic[2][1][2]]])

        for name, error in (
            ("BadEval", "field has no binding"), ("DomainEval", "expression form is not implemented"),
            ("LateError", "division by zero"), ("Cycle", "cycle"),
            ("ParamDomain", "parameter mismatch"), ("MissingRemote", "not implemented"),
            ("ProviderRemote", "not implemented"),
        ):
            check(profile + ":refuse:" + name, mapped + ["SELECT * FROM " + name + " IN 'one'"], refusal=error)
        check(profile + ":undeclared-parameter", mapped + [
            "--param", "Floor=long:2", "PARAMETERS Floor LONG; SELECT * FROM Undeclared IN 'one'"
        ], refusal="name has no binding")
        check(profile + ":linked-base-guard", mapped + ["SELECT * FROM LinkedItems IN 'one'"],
              refusal="not implemented")
        check(profile + ":missing-parameter", mapped + ["SELECT * FROM Param IN 'one'"], refusal="parameter mismatch")
        check(profile + ":missing-target", run + ["SELECT * FROM Nested IN 'one'"], refusal="not implemented")
        check(profile + ":missing-third-token", run + ["--external", "one", database["one"],
              "SELECT * FROM Redirect IN 'one'"], refusal="not implemented")
        check(profile + ":duplicate-token", mapped + ["--external", "one", database["one"],
              "SELECT * FROM Nested IN 'one'"], refusal="invalid query run parameter")
        for option in ("--native-bytes", "--native-rows", "--native-steps", "--execution-bytes",
                       "--execution-rows", "--execution-steps", "--output-bytes"):
            error = "output limit" if option == "--output-bytes" else "resource limit"
            check(profile + ":positive-runtime-limit:" + option,
                  mapped + [option, "1", "SELECT * FROM Nested IN 'one'"], refusal=error)

    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    report = {"scope": "Original inert saved metadata and getter-modeled explicit target execution; "
                       "not native Microsoft Access namespace qualification.",
              "controls": len(observations), "modeled_cells": modeled_cells,
              "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"controls": len(observations), "modeled_cells": modeled_cells,
                      "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
