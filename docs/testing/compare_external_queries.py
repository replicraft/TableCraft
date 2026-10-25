"""Independent local-getter model for explicit host-mapped external IN queries.

The reference reads ordinary local tables only. SQL lexical rules are explicit
engineering contracts; neither reference SQL nor imported content is executed.
"""

import argparse
from collections import defaultdict
import csv
from datetime import datetime, timezone
from io import StringIO
import json
from pathlib import Path
import subprocess
import sys

from compare_linked_cli import display
from compare_linked_queries import JACKCESS_SHA256, PROFILES, ROOT, digest, java, model, output, value


TOKEN = "remote-token"
REFERENCES = [
    "https://learn.microsoft.com/en-us/office/vba/access/concepts/miscellaneous/in-clause-microsoft-access-sql",
    "https://support.microsoft.com/en-gb/access/from-clause",
]


def records(text):
    tables = defaultdict(list)
    policies = {}
    indexes = {}
    for line in text.splitlines():
        fields = line.split("\t")
        if fields[0] == "R":
            tables[(fields[1], value(fields[2]))].append(list(map(value, fields[3:])))
        elif fields[0] == "P":
            policies[(fields[1], value(fields[2]), value(fields[3]))] = tuple(map(int, fields[4:]))
        elif fields[0] == "X":
            key = (fields[1], value(fields[2]), value(fields[3]))
            assert key not in indexes
            indexes[key] = (value(fields[4]), fields[5])
        else:
            assert fields == ["O", "0"]
    assert indexes == {
        ("root", "Remote People", "RootOnlyIndex"): ("ID", "true"),
        ("target", "Remote People", "ExternalId"): ("ID", "true"),
    }
    return tables, policies


def cases(tables, comparison):
    people = tables[("target", "Remote People")]
    local_people = tables[("root", "Remote People")]
    orders = tables[("target", "Remote Orders")]
    root_orders = tables[("root", "Remote Orders")]
    rates = tables[("target", "Rates")]
    root_rates = tables[("root", "Rates")]
    by_id = {person[0]: person for person in people}
    by_team = {rate[0]: rate for rate in rates}
    root_by_team = {rate[0]: rate for rate in root_rates}
    counts = defaultdict(int)
    root_counts = defaultdict(int)
    for order in orders:
        counts[order[1]] += 1
    for order in root_orders:
        root_counts[order[1]] += 1
    fold = str.lower if comparison == "database" else lambda text: text
    grouped = defaultdict(list)
    for order in orders:
        if order[1] in by_id:
            grouped[by_id[order[1]][1]].append(order[2])
    group_rows = []
    for team, units in sorted(grouped.items()):
        present = [unit for unit in units if unit is not None]
        group_rows.append([team, len(units), sum(present) if present else None])

    result = [
        ("root_people", "SELECT ID,Team,Label FROM [Remote People] ORDER BY ID",
         ["ID", "Team", "Label"], local_people),
        ("external_people", f"SELECT ID,Team,Label FROM [Remote People] IN '{TOKEN}' ORDER BY ID",
         ["ID", "Team", "Label"], people),
        ("root_rates", "SELECT Team,Rate,Label FROM Rates ORDER BY Team", ["Team", "Rate", "Label"], root_rates),
        ("external_rates", f"SELECT Team,Rate,Label FROM Rates IN '{TOKEN}' ORDER BY Team",
         ["Team", "Rate", "Label"], rates),
        ("external_alias", f"SELECT p.ID,p.Label FROM [Remote People] p IN '{TOKEN}' ORDER BY p.ID",
         ["ID", "Label"], [[person[0], person[2]] for person in people]),
        ("external_carrier", f"SELECT ID,VarType(ID) AS K FROM [Remote People] IN '{TOKEN}' ORDER BY ID",
         ["ID", "K"], [[person[0], 3] for person in people]),
        ("filter", f"SELECT ID FROM [Remote People] IN '{TOKEN}' WHERE Team=10 ORDER BY ID",
         ["ID"], [[person[0]] for person in people if person[1] == 10]),
        ("text", f"SELECT ID FROM [Remote People] IN '{TOKEN}' WHERE Label='alpha' ORDER BY ID",
         ["ID"], [[person[0]] for person in people if person[2] is not None and fold(person[2]) == "alpha"]),
        ("empty", f"SELECT ID FROM [Remote Empty] IN '{TOKEN}'", ["ID"], []),
        ("empty_count", f"SELECT Count(*) AS N FROM [Remote Empty] IN '{TOKEN}'", ["N"], [[0]]),
        ("inner", "SELECT p.ID,o.ID AS OrderID,o.Units FROM ([Remote People] p "
         f"INNER JOIN [Remote Orders] o ON p.ID=o.Person) IN '{TOKEN}' ORDER BY o.ID",
         ["ID", "OrderID", "Units"],
         [[order[1], order[0], order[2]] for order in orders if order[1] in by_id]),
        ("comma", "SELECT p.ID,o.ID AS OrderID,o.Units FROM [Remote People] p,[Remote Orders] o "
         f"IN '{TOKEN}' WHERE p.ID=o.Person ORDER BY o.ID", ["ID", "OrderID", "Units"],
         [[order[1], order[0], order[2]] for order in orders if order[1] in by_id]),
        ("all_direct_leaves", "SELECT p.ID,r.Rate FROM ([Remote People] p "
         f"INNER JOIN Rates r ON p.Team=r.Team) IN '{TOKEN}' ORDER BY p.ID",
         ["ID", "Rate"], [[person[0], by_team[person[1]][1]] for person in people if person[1] in by_team]),
        ("groups", "SELECT p.Team,Count(*) AS N,Sum(o.Units) AS Total FROM ([Remote People] p "
         f"INNER JOIN [Remote Orders] o ON p.ID=o.Person) IN '{TOKEN}' GROUP BY p.Team ORDER BY p.Team",
         ["Team", "N", "Total"], group_rows),
        ("derived_defaults_root", "SELECT p.ID,r.Rate FROM ([Remote People] p "
         "INNER JOIN (SELECT Team,Rate FROM Rates) r ON p.Team=r.Team) "
         f"IN '{TOKEN}' ORDER BY p.ID", ["ID", "Rate"],
         [[person[0], root_by_team[person[1]][1]] for person in people if person[1] in root_by_team]),
        ("scalar_defaults_root", "SELECT p.ID,(SELECT Count(*) FROM [Remote Orders] o "
         f"WHERE o.Person=p.ID) AS N FROM [Remote People] p IN '{TOKEN}' ORDER BY p.ID",
         ["ID", "N"], [[person[0], root_counts[person[0]]] for person in people]),
        ("scalar_own_in", "SELECT p.ID,(SELECT Count(*) FROM [Remote Orders] o "
         f"IN '{TOKEN}' WHERE o.Person=p.ID) AS N FROM [Remote People] p IN '{TOKEN}' ORDER BY p.ID",
         ["ID", "N"], [[person[0], counts[person[0]]] for person in people]),
        ("root_outer_external_subquery", "SELECT p.ID,(SELECT Count(*) FROM [Remote Orders] o "
         f"IN '{TOKEN}' WHERE o.Person=p.ID) AS N FROM [Remote People] p ORDER BY p.ID",
         ["ID", "N"], [[person[0], counts[person[0]]] for person in local_people]),
        ("predicate_own_in", "SELECT p.ID FROM [Remote People] p WHERE EXISTS "
         f"(SELECT o.ID FROM [Remote Orders] o IN '{TOKEN}' WHERE o.Person=p.ID) ORDER BY p.ID",
         ["ID"], [[person[0]] for person in local_people if counts[person[0]]]),
        ("root_named_external_subquery", "SELECT Person,(SELECT Count(*) FROM [Remote Orders] "
         f"IN '{TOKEN}') AS N FROM [Linked root] ORDER BY Person", ["Person", "N"],
         [[person[0], len(orders)] for person in people if person[0] >= 2]),
        ("domain_defaults_root", "SELECT ID,DCount('*','Remote People') AS N "
         f"FROM [Remote People] IN '{TOKEN}' ORDER BY ID", ["ID", "N"],
         [[person[0], len(local_people)] for person in people]),
    ]
    joined = {"inner", "all_direct_leaves", "groups", "derived_defaults_root"}
    for name, sql, columns, expected in list(result):
        if name in joined:
            ungrouped = sql.replace("FROM (", "FROM ", 1).replace(") IN", " IN", 1)
            assert ungrouped != sql
            result.append((name + "_ungrouped", ungrouped, columns, expected))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--reader", type=Path)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--source", type=Path, action="append", default=[])
    args = parser.parse_args()
    assert args.reader or args.cli, "Supply at least one actual executable"
    assert not args.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    paths = {
        args.jar, Path(__file__), ROOT / "docs/testing/compare_linked_queries.py",
        ROOT / "docs/testing/compare_linked_cli.py", ROOT / "docs/testing/LinkedQueryFixture.java",
        ROOT / "docs/testing/LinkedQueryOracle.java", ROOT / "docs/testing/ExternalQueryFixture.java",
        ROOT / "docs/testing/ExternalQueryOracle.java", *args.source,
    }
    paths.update(path for path in (args.reader, args.cli) if path is not None)
    initial_hashes = {path: digest(path) for path in paths}
    observations = []
    counts = defaultdict(int)
    cells = defaultdict(int)

    def run(kind, frontend, backend, sql, mappings, comparison="ordinal", extra=(), expected=None):
        if kind == "engineering":
            command = [args.reader, frontend, backend, sql, "--external", TOKEN, "--collation", comparison]
            for mapping in mappings:
                command += ["--mapping", *mapping]
        else:
            command = [args.cli, "query", "run", sql, "--database", frontend, "--format", "tsv",
                       "--external", TOKEN, backend, "--collation", comparison]
            for mapping in mappings:
                command += ["--link", mapping[0], backend]
        command += list(extra)
        command = list(map(str, command))
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=240)
        observations.append({
            "kind": kind, "command": command, "returncode": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr,
        })
        counts[kind] += 1
        if expected is None:
            assert result.returncode != 0 and not result.stdout, (command, result)
            return
        assert result.returncode == 0 and not result.stderr, (command, result)
        columns, rows = expected
        if kind == "engineering":
            actual = output(result.stdout)
            assert actual == (columns, rows), (command, actual, expected)
        else:
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            wanted = [columns, *[[display(cell) for cell in row] for row in rows]]
            assert actual == wanted, (command, actual, wanted)
        cells[kind] += sum(map(len, rows))

    originals = {}
    profile_results = []
    for profile in PROFILES:
        directory = args.directory / profile
        root_format = "V2000" if profile == "Mixed" else profile
        target_format = "V2016" if profile == "Mixed" else profile
        frontend = directory / ("frontend.mdb" if root_format == "V2000" else "frontend.accdb")
        backend = directory / ("backend.mdb" if target_format == "V2000" else "backend.accdb")
        for path in (frontend, backend):
            paths.add(path)
            originals[path] = digest(path)
        getter = directory / "getter.tsv"
        paths.add(getter)
        originals[getter] = digest(getter)
        raw = java(args.jar, "ExternalQueryOracle.java", frontend, backend)
        assert raw == getter.read_text(encoding="utf-8"), "Getter replay differs from original captured records"
        tables, policies = records(raw)
        linked = [line.split("\t") for line in java(
            args.jar, "LinkedQueryOracle.java", frontend, backend,
        ).splitlines()]
        _, _, _, _, mappings = model(linked)
        profile_results.append({"profile": profile, "getter_records": raw, "link_getter_records": linked})
        for kind, binary in [("engineering", args.reader), ("cli", args.cli)]:
            if binary is None:
                continue
            for comparison in ("ordinal", "database"):
                for name, sql, columns, expected in cases(tables, comparison):
                    run(kind, frontend, backend, sql, mappings, comparison, expected=(columns, expected))
                mixed = "SELECT p.ID,r.Team FROM ([Remote People] p "
                mixed += "INNER JOIN (SELECT Team,Label FROM Rates) r ON p.Label=r.Label) "
                mixed += f"IN '{TOKEN}' ORDER BY p.ID,r.Team"
                incompatible = policies[("target", "Remote People", "Label")] != policies[("root", "Rates", "Label")]
                if comparison == "database" and incompatible:
                    run(kind, frontend, backend, mixed, mappings, comparison)
                else:
                    fold = str.lower if comparison == "database" else lambda text: text
                    expected = [[person[0], rate[0]] for person in tables[("target", "Remote People")]
                                for rate in tables[("root", "Rates")] if person[2] is not None
                                and fold(person[2]) == fold(rate[2])]
                    run(kind, frontend, backend, mixed, mappings, comparison,
                        expected=(["ID", "Team"], expected))
            for sql in [
                f"SELECT ID FROM [Root Only] IN '{TOKEN}'",
                f"SELECT Person FROM [Linked root] IN '{TOKEN}'",
                f"SELECT ID FROM [Remote chain] IN '{TOKEN}'",
                "SELECT ID FROM [Remote People] IN 'unknown-token'",
                f"SELECT ID FROM [Remote People] IN '{TOKEN}' 'ODBC;'",
                f"SELECT ID FROM [Remote People] IN '{TOKEN}' 'UNKNOWN;'",
                f"SELECT ID FROM [Remote People] IN '{TOKEN.upper()}'",
                f"SELECT ID,10/(ID-6) AS V FROM [Remote People] IN '{TOKEN}' ORDER BY ID",
                f"SELECT ID FROM [Remote People] IN '{TOKEN}' WHERE ID<6 AND 10/(ID-6)>0",
                f"SELECT ID FROM [Remote Empty] IN '{TOKEN}' WHERE EXISTS "
                "(SELECT ID FROM [Remote People] IN 'unknown-token')",
            ]:
                run(kind, frontend, backend, sql, mappings)
            duplicate = ["--external", TOKEN] if kind == "engineering" else ["--external", TOKEN, backend]
            sql = f"SELECT ID FROM [Remote People] IN '{TOKEN}' ORDER BY ID"
            run(kind, frontend, backend, sql, mappings, extra=duplicate)
            limits = ["--native-bytes", "--native-rows", "--native-steps", "--output-bytes"]
            limits.append("--query-rows" if kind == "engineering" else "--execution-rows")
            for option in limits:
                run(kind, frontend, backend, sql, mappings, extra=(option, "1"))
            # The explicit host binding, not the token's spelling, chooses bytes.
            local_people = tables[("root", "Remote People")]
            run(kind, frontend, frontend, sql, [], expected=(
                ["ID"], [[person[0]] for person in local_people]
            ))
            # A file that exists is still unavailable without its exact binding.
            run(kind, frontend, backend, f"SELECT ID FROM [Remote People] IN '{backend}'", mappings)
            if kind == "cli":
                expected = (["ID"], [[person[0]] for person in tables[("target", "Remote People")]])
                run(kind, frontend, backend, sql, mappings,
                    extra=("--external-index", TOKEN, "Remote People", "ExternalId"), expected=expected)
                run(kind, frontend, backend, sql, mappings,
                    extra=("--index", "Remote People", "RootOnlyIndex"), expected=expected)
                for token, index in [(TOKEN, "MissingIndex"), (TOKEN, "RootOnlyIndex"), ("unknown", "ExternalId")]:
                    run(kind, frontend, backend, sql, mappings,
                        extra=("--external-index", token, "Remote People", index))
        for path, expected in originals.items():
            assert digest(path) == expected, path

    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    for path, expected in initial_hashes.items():
        assert digest(path) == expected, path
    for path, expected in originals.items():
        assert hashes[str(path.resolve().relative_to(ROOT))] == expected
    report = {
        "scope": "Explicit external IN engineering lexical rules; native Access execution unqualified",
        "references": REFERENCES,
        "runner_arguments": sys.argv[1:],
        "date_utc": datetime.now(timezone.utc).isoformat(),
        "controls": dict(counts), "model_cells": dict(cells), "hashes": hashes,
        "profiles": profile_results, "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"controls": dict(counts), "model_cells": dict(cells), "hashes": len(hashes)}))


if __name__ == "__main__":
    main()
