"""Independent getter model for explicitly mapped one-hop native queries.

Every backend path is a host argument. Stored Database strings only pin metadata;
the oracle refuses link traversal and never executes SQL or imported content.
Controlled catalog copies are refusal/authority checks, not native writer evidence.
"""

import argparse
import base64
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PROFILES = ("V2000", "V2007", "V2016", "V2019", "Mixed")
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(*arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True,
                            text=True, encoding="utf-8", timeout=240)
    assert result.returncode == 0, (arguments, result.stderr)
    return result.stdout


def java(jar, source, *arguments):
    return execute("java", "-Dstdout.encoding=UTF-8", "-cp", jar,
                   RECIPES / source, *arguments)


def value(tag):
    if tag == "N":
        return None
    if tag.startswith("I"):
        return int(tag[1:])
    if tag.startswith("T"):
        return base64.b64decode(tag[1:], validate=True).decode("utf-8")
    if tag.startswith("F"):
        return struct.unpack(">d", bytes.fromhex(tag[1:]))[0]
    raise AssertionError(("Unknown engineering cell", tag))


def output(text):
    columns = []
    rows = []
    for line in text.splitlines():
        fields = line.split("\t")
        if fields[0] == "C":
            assert int(fields[1]) == len(columns)
            columns.append(value(fields[2]))
        else:
            assert fields[0] == "R" and len(fields) == 4
            row, column = map(int, fields[1:3])
            if row == len(rows):
                rows.append([])
            assert row == len(rows) - 1 and column == len(rows[row])
            rows[row].append(value(fields[3]))
    assert all(len(row) == len(columns) for row in rows)
    return columns, rows


def model(records):
    links = {}
    identities = {}
    tables = defaultdict(list)
    policies = {}
    saved = {}
    for fields in records:
        tag = fields[0]
        if tag == "L":
            links[value(fields[1])] = (int(fields[2]), *map(value, fields[3:]))
        elif tag == "D":
            identities[value(fields[1])] = int(fields[2])
        elif tag == "R":
            tables[value(fields[1])].append(list(map(value, fields[2:])))
        elif tag == "P":
            policies[(value(fields[1]), value(fields[2]))] = tuple(map(int, fields[3:]))
        elif tag == "Q":
            saved[value(fields[1])] = value(fields[2])
        else:
            assert fields == ["O", "0"]
    assert set(links) == {"People link", "People alias", "Orders link", "Empty link", "Unmapped link"}
    assert set(identities) == {"Remote People", "Remote Orders", "Remote Empty", "Remote chain"}
    assert set(saved) == {"Linked root"}
    assert "[People link]" in saved["Linked root"]
    mappings = [[alias, str(native), database, target, str(identities[target])]
                for alias, (native, database, target, connect) in links.items()
                if alias != "Unmapped link"]
    assert all(link[3] is None for link in links.values())
    return links, identities, tables, policies, mappings


def cases(tables, comparison):
    people = tables["Remote People"]
    orders = tables["Remote Orders"]
    rates = tables["Rates"]
    people_by_id = {row[0]: row for row in people}
    rates_by_team = {row[0]: row for row in rates}
    orders_by_person = defaultdict(list)
    for order in orders:
        if order[1] is not None:
            orders_by_person[order[1]].append(order)
    saved = [[person[0], person[2]] for person in people if person[0] >= 2]
    inner = [[people_by_id[order[1]][0], order[0], order[2], people_by_id[order[1]][2]]
             for order in orders if order[1] in people_by_id]
    left = []
    for person in people:
        matching = orders_by_person[person[0]]
        left.extend([[person[0], order[0], order[2]] for order in matching]
                    if matching else [[person[0], None, None]])
    right = [[people_by_id[order[1]][0] if order[1] in people_by_id else None, order[0]]
             for order in orders]
    groups = defaultdict(list)
    for order in orders:
        if order[1] in people_by_id:
            groups[people_by_id[order[1]][1]].append(order[2])
    grouped = []
    for team, units in sorted(groups.items()):
        present = [unit for unit in units if unit is not None]
        grouped.append([team, len(units), len(present), sum(present) if present else None])
    fold = (lambda value: value) if comparison == "ordinal" else str.lower
    equal = [[person[0]] for person in people
             if person[2] is not None and fold(person[2]) == "alpha"]
    mixed = [[person[0], person[2], rates_by_team[person[1]][1]]
             for person in people if person[1] in rates_by_team]
    return [
        ("local", "SELECT Team,Rate,Label FROM Rates ORDER BY Team", ["Team", "Rate", "Label"], rates),
        ("people", "SELECT ID,Team,Label FROM [People link] ORDER BY ID", ["ID", "Team", "Label"], people),
        ("alias", "SELECT ID,Label FROM [People alias] ORDER BY ID", ["ID", "Label"],
         [[person[0], person[2]] for person in people]),
        ("orders", "SELECT ID,Person,Units FROM [Orders link] ORDER BY ID", ["ID", "Person", "Units"], orders),
        ("empty", "SELECT ID FROM [Empty link] ORDER BY ID", ["ID"], []),
        ("empty_count", "SELECT Count(*) AS N FROM [Empty link]", ["N"], [[0]]),
        ("filter", "SELECT ID FROM [People link] WHERE Team=10 ORDER BY ID", ["ID"],
         [[person[0]] for person in people if person[1] == 10]),
        ("text", "SELECT ID FROM [People link] WHERE Label='alpha' ORDER BY ID", ["ID"], equal),
        ("inner", "SELECT p.ID,o.ID AS OrderID,o.Units,p.Label FROM [People link] p "
         "INNER JOIN [Orders link] o ON p.ID=o.Person ORDER BY o.ID",
         ["ID", "OrderID", "Units", "Label"], inner),
        ("left", "SELECT p.ID,o.ID AS OrderID,o.Units FROM [People link] p "
         "LEFT JOIN [Orders link] o ON p.ID=o.Person ORDER BY p.ID,o.ID",
         ["ID", "OrderID", "Units"], left),
        ("right", "SELECT p.ID,o.ID AS OrderID FROM [People link] p "
         "RIGHT JOIN [Orders link] o ON p.ID=o.Person ORDER BY o.ID", ["ID", "OrderID"], right),
        ("grouped", "SELECT p.Team,Count(*) AS N,Count(o.Units) AS Present,Sum(o.Units) AS Total "
         "FROM [People link] p INNER JOIN [Orders link] o ON p.ID=o.Person "
         "GROUP BY p.Team ORDER BY p.Team", ["Team", "N", "Present", "Total"], grouped),
        ("mixed_numeric", "SELECT p.ID,p.Label,r.Rate FROM [People link] p "
         "INNER JOIN Rates r ON p.Team=r.Team ORDER BY p.ID", ["ID", "Label", "Rate"], mixed),
        ("same_target", "SELECT a.ID,b.Label FROM [People link] a "
         "INNER JOIN [People alias] b ON a.ID=b.ID ORDER BY a.ID", ["ID", "Label"],
         [[person[0], person[2]] for person in people]),
        ("named_root", "SELECT Person,Label FROM [Linked root] ORDER BY Person", ["Person", "Label"], saved),
        ("saved_root", None, ["Person", "Label"], saved),
        ("correlated", "SELECT p.ID,(SELECT Count(*) FROM [Orders link] o WHERE o.Person=p.ID) AS N "
         "FROM [People link] p ORDER BY p.ID", ["ID", "N"],
         [[person[0], len(orders_by_person[person[0]])] for person in people]),
    ]


def command(reader, frontend, backend, sql, mappings, comparison="ordinal", extra=()):
    arguments = [reader, frontend, backend]
    arguments.extend([sql] if sql is not None else ["--saved", "Linked root"])
    arguments.extend(["--collation", comparison])
    for mapping in mappings:
        arguments.extend(["--mapping", *mapping])
    return [*arguments, *extra]


def refused(arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True, timeout=240)
    assert result.returncode != 0 and not result.stdout, (arguments, result.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "reader", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    paths = {
        args.jar, args.reader, Path(__file__),
        RECIPES / "LinkedQueryFixture.java", RECIPES / "LinkedQueryOracle.java",
        ROOT / "crates/backends/tablecraft-access/examples/linked_query.rs",
        *[ROOT / "crates/backends/tablecraft-access/src/source" / path for path in (
            "handle.rs", "links/mod.rs", "links/mapping.rs", "links/forwarding.rs",
            "links/error.rs", "budget.rs", "collation.rs", "inventory.rs",
        )],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    cells = 0
    observations = []
    for profile in PROFILES:
        directory = args.directory / profile
        frontend_format, backend_format = ("V2000", "V2016") if profile == "Mixed" else (profile, profile)
        if args.generate:
            java(args.jar, "LinkedQueryFixture.java", directory, frontend_format, backend_format)
        frontend = directory / ("frontend.mdb" if frontend_format == "V2000" else "frontend.accdb")
        backend = directory / ("backend.mdb" if backend_format == "V2000" else "backend.accdb")
        original_hashes = {frontend: digest(frontend), backend: digest(backend)}
        records = [line.split("\t") for line in java(
            args.jar, "LinkedQueryOracle.java", frontend, backend,
        ).splitlines()]
        links, identities, tables, policies, mappings = model(records)
        assert records[-1] == ["O", "0"]
        controls += 1
        for comparison in ("ordinal", "database"):
            for name, sql, columns, expected in cases(tables, comparison):
                actual = output(execute(*command(args.reader, frontend, backend, sql, mappings, comparison)))
                assert actual == (columns, expected), (profile, comparison, name, actual, expected)
                controls += 1
                cells += sum(map(len, expected))
            sql = "SELECT p.ID,r.Team FROM [People link] p INNER JOIN Rates r " \
                  "ON p.Label=r.Label ORDER BY p.ID,r.Team"
            conflict = policies[("Remote People", "Label")] != policies[("Rates", "Label")]
            if comparison == "database" and conflict:
                refused(command(args.reader, frontend, backend, sql, mappings, comparison))
            else:
                fold = (lambda value: value) if comparison == "ordinal" else str.lower
                expected = [[person[0], rate[0]] for person in tables["Remote People"]
                            for rate in tables["Rates"] if person[2] is not None
                            and fold(person[2]) == fold(rate[2])]
                actual = output(execute(*command(args.reader, frontend, backend, sql, mappings, comparison)))
                assert actual == (["ID", "Team"], expected)
                cells += sum(map(len, expected))
            controls += 1
        local = "SELECT Team,Rate,Label FROM Rates ORDER BY Team"
        assert output(execute(*command(args.reader, frontend, backend, local, []))) == (
            ["Team", "Rate", "Label"], tables["Rates"])
        controls += 1
        refused(command(args.reader, frontend, backend, "SELECT ID FROM [Unmapped link]", mappings))
        refused(command(args.reader, frontend, backend, "SELECT ID FROM [People link]", []))
        refused(command(args.reader, frontend, backend, "SELECT ID FROM [Remote People]", mappings))
        controls += 3
        for extra in (["--native-bytes", "0"], ["--native-rows", "0"],
                      ["--native-steps", "0"], ["--query-rows", "0"], ["--output-bytes", "0"]):
            refused(command(args.reader, frontend, backend, "SELECT ID FROM [People link]", mappings, extra=extra))
            controls += 1
        refused(command(args.reader, frontend, backend, "SELECT 1/(ID-4) FROM [People link]", mappings))
        refused(command(args.reader, frontend, backend, "SELECT Missing FROM [People link]", mappings))
        controls += 2
        for field, replacement in ((0, "Rates"), (0, "Unknown alias"), (1, "123"),
                                   (2, "unexpected.accdb"), (3, "Remote Orders"), (4, "-2147483648")):
            changed = [mapping.copy() for mapping in mappings]
            changed[0][field] = replacement
            refused(command(args.reader, frontend, backend, local, changed))
            controls += 1
        refused(command(args.reader, frontend, backend, local, [*mappings, mappings[0]]))
        controls += 1
        for change in ("provider", "odbc", "database", "identity", "chain"):
            copy = directory / f"control-{change}{frontend.suffix}"
            if args.generate:
                java(args.jar, "LinkedQueryFixture.java", "--metadata-copy",
                     frontend, copy, "People link", change)
            refused(command(args.reader, copy, backend, "SELECT ID FROM [People link]", mappings))
            controls += 1
            assert output(execute(*command(args.reader, copy, backend, local, []))) == (
                ["Team", "Rate", "Label"], tables["Rates"])
            controls += 1
            if change in ("database", "chain"):
                changed = [mapping.copy() for mapping in mappings]
                selected = next(mapping for mapping in changed if mapping[0] == "People link")
                if change == "database":
                    selected[2] = "unexpected.accdb"
                    actual = output(execute(*command(args.reader, copy, backend,
                        "SELECT ID,Team,Label FROM [People link] ORDER BY ID", changed)))
                    assert actual == (["ID", "Team", "Label"], tables["Remote People"])
                else:
                    selected[3] = "Remote chain"
                    selected[4] = str(identities["Remote chain"])
                    refused(command(args.reader, copy, backend, local, changed))
                controls += 1
            hashes[str(copy.resolve().relative_to(ROOT))] = digest(copy)
        for path, expected in original_hashes.items():
            assert digest(path) == expected, path
            hashes[str(path.resolve().relative_to(ROOT))] = expected
        observations.append({"profile": profile, "policies": {
            "/".join(key): order for key, order in policies.items()
        }, "supplied_mappings": len(mappings), "unmapped_links": 1,
            "reference_link_resolver_calls": 0, "originals_unchanged": True})
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    args.report.write_text(json.dumps({
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "controls": controls, "compared_result_cells": cells,
        "hashes": hashes, "profiles": observations,
        "scope": "Explicit host mappings and independent getter model; no native linked SQL qualification",
    }, indent=2) + "\n")
    print(f"Passed {controls} explicit linked controls, {cells} result cells, {len(hashes)} unchanged hashes")


if __name__ == "__main__":
    main()
