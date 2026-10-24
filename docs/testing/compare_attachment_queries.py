"""Compare aligned attachment queries to inert independent stored records.

Uses the original AttachmentFixture recipe and Jackcess reader; this is an
explicit flattening/membership model, not a Microsoft Access SQL oracle.
"""
import argparse
from pathlib import Path
import tempfile

from compare_browsing import RECIPES, display, run
from compare_cached_pages import date_display
from compare_large_tables import digest
from compare_multivalue_queries import rejected
from compare_multivalue_parent_criteria import count_grid

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2007", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--collation", choices=("ordinal", "database"), default="ordinal")
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256, "Expected reviewed Jackcess artifact"
    cli_before = digest(args.cli)
    controls = 0
    for path in [args.ace2007, args.ace2016, args.ace2019]:
        before = digest(path)
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "AttachmentFixture.java", "--query-metadata", path)
        parents, children, child_ids, encoded_payloads = {}, {}, {}, {}
        for line in tagged.splitlines():
            cells = line.split("\t")
            if cells[0] == "P":
                assert cells[1] not in parents
                parents[cells[1]] = cells[2]
                children[cells[1]] = []
                child_ids[cells[1]] = []
                encoded_payloads[cells[1]] = []
            else:
                assert len(cells) == 8 and cells[0] in parents
                children[cells[0]].append([cells[2], cells[3], cells[4], cells[6], cells[7]])
                child_ids[cells[0]].append(cells[1])
                encoded_payloads[cells[0]].append(cells[5])
        assert len(parents) == 2 and sum(map(len, children.values())) == 6
        options = ["--database", path, "--collections", "--native-bytes", "67108864", "--collation", args.collation]
        names = ["FileURL", "FileName", "FileType", "FileTimeStamp", "FileFlags"]
        for positions in [[i] for i in range(len(names))] + [list(range(len(names))), [2, 1, 4]]:
            selected = [names[i] for i in positions]
            sql = "SELECT ID," + ",".join("Files." + name + " AS " + name for name in selected) + " FROM Collections"
            expected = "ID\t" + "\t".join(selected) + "\n"
            for parent, identity in parents.items():
                for child in children[parent] or [["N"] * len(names)]:
                    expected += display(identity) + "\t" + "\t".join(date_display(child[i]) if i == 3 else display(child[i]) for i in positions) + "\n"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
        expected = "ID\tN\n" + "".join(display(identity) + "\t" + str(sum(row[1] != "N" for row in children[parent])) + "\n" for parent, identity in parents.items())
        assert run(args.cli, "query", "run", "SELECT ID,Count(Files.FileName) AS N FROM Collections GROUP BY ID ORDER BY ID", *options) == expected
        controls += 1
        # Displayed members filter the same child, retaining its paired type.
        chosen = "T" + "txt".encode().hex()
        expected = "ID\tName\tKind\n" + "".join(display(parents[parent]) + "\t" + display(row[1]) + "\t" + display(row[2]) + "\n" for parent in parents for row in children[parent] if row[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileType AS Kind FROM Collections WHERE Files.FileType='txt'", *options) == expected
        controls += 1
        # Hidden member criteria retain each parent at most once.
        expected = "ID\n" + "".join(display(identity) + "\n" for parent, identity in parents.items() if any(row[2] == chosen for row in children[parent]))
        assert run(args.cli, "query", "run", "SELECT ID FROM Collections WHERE Files.FileType='txt' ORDER BY ID", *options) == expected
        controls += 1
        # Unprojected sort/HAVING members still refer to aligned child records.
        grouped = [(display(identity), max(1, len(children[parent])), sum(row[1] != "N" for row in children[parent])) for parent, identity in parents.items()]
        expected = "ID\tTotal\n" + "".join(f"{identity}\t{total}\n" for identity, total, present in grouped if present)
        sql = "SELECT ID,Count(*) AS Total FROM Collections GROUP BY ID HAVING Count(Files.FileName)>0 ORDER BY Count(Files.FileType) DESC,ID"
        assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
        controls += 1
        flat = [(display(parents[parent]), row) for parent in parents for row in (children[parent] or [["N"] * len(names)])]
        for descending in (False, True):
            ordered = sorted(flat, key=lambda pair: (pair[1][4] != "N", int(display(pair[1][4])) if pair[1][4] != "N" else 0), reverse=descending)
            for matching in (False, True):
                expected = "ID\n" + "".join(identity + "\n" for identity, row in ordered if not matching or row[2] == chosen)
                sql = "SELECT ID FROM Collections" + (" WHERE Files.FileType='txt'" if matching else "") + " ORDER BY Files.FileFlags" + (" DESC" if descending else "")
                assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
                controls += 1
        # Explicit logical decoder view. Public getFileData() supplies original
        # bytes; this deliberately does not claim native SQL representation.
        decoded = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "AttachmentFixture.java", "--inspect", path)
        payloads = {parent: [] for parent in parents}
        decoded_ids = {parent: [] for parent in parents}
        for line in decoded.splitlines():
            parent, child_id, _, payload = line.split("\t")
            payloads[parent].append("X" + payload)
            decoded_ids[parent].append(child_id)
        assert decoded_ids == child_ids
        assert all(len(payloads[p]) == len(children[p]) for p in parents)
        # Check the physical OLE declaration and raw flat carrier independently
        # of the public encoded getter used by the original metadata recipe.
        physical = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "AttachmentStoredDataOracle.java", path)
        declaration, *physical_rows = physical.splitlines()
        assert declaration == "D\tFileData\tOLE\tBinary"
        expected_physical = ["\t".join(["C", parent, identity, value]) for parent in parents for identity, value in zip(child_ids[parent], encoded_payloads[parent], strict=True)]
        assert physical_rows == expected_physical
        controls += 1
        assert any(encoded_payloads[p][i] != value for p in parents for i, value in enumerate(payloads[p]))
        controls += 1
        # Default FileData exposes exact opaque storage, preserving alignment in
        # either projection order. The decoded view remains an explicit choice.
        for positions in [[0], [1, 0], [0, 1, 2], [2, 1, 0]]:
            raw_names = ["FileData", "FileName", "FileType"]
            selected = [raw_names[i] for i in positions]
            sql = "SELECT ID," + ",".join("Files." + name + " AS " + name for name in selected) + " FROM Collections"
            expected = "ID\t" + "\t".join(selected) + "\n"
            for parent, identity in parents.items():
                for index, child in enumerate(children[parent] or [["N"] * len(names)]):
                    values = [encoded_payloads[parent][index] if children[parent] else "N", child[1], child[2]]
                    expected += display(identity) + "\t" + "\t".join(display(values[i]) for i in positions) + "\n"
            assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
            controls += 1
        expected = "ID\tN\n" + "".join(display(identity) + "\t" + str(len(encoded_payloads[parent])) + "\n" for parent, identity in parents.items())
        assert run(args.cli, "query", "run", "SELECT ID,Count(Files.FileData) AS N FROM Collections GROUP BY ID ORDER BY ID", *options) == expected
        controls += 1
        decoded_options = [*options, "--decoded-attachments"]
        for with_names in [False, True]:
            sql = "SELECT ID," + ("Files.FileName AS Name," if with_names else "") + "Files.FileData AS Data FROM Collections"
            expected = "ID\t" + ("Name\t" if with_names else "") + "Data\n"
            for parent, identity in parents.items():
                for index, child in enumerate(children[parent] or [["N"] * len(names)]):
                    payload = payloads[parent][index] if children[parent] else "N"
                    expected += display(identity) + "\t" + (display(child[1]) + "\t" if with_names else "") + display(payload) + "\n"
            assert run(args.cli, "query", "run", sql, *decoded_options) == expected
            controls += 1
        expected = "ID\tN\n" + "".join(display(identity) + "\t" + str(len(payloads[parent])) + "\n" for parent, identity in parents.items())
        assert run(args.cli, "query", "run", "SELECT ID,Count(Files.FileData) AS N FROM Collections GROUP BY ID ORDER BY ID", *decoded_options) == expected
        controls += 1
        # Existing explicit parent policy negates positive NULL membership; an
        # empty list therefore matches IS NOT NULL. This is not a native oracle.
        expected = "ID\n" + "".join(display(identity) + "\n" for parent, identity in parents.items() if all(value != "N" for value in payloads[parent]))
        assert run(args.cli, "query", "run", "SELECT ID FROM Collections WHERE Files.FileData IS NOT NULL ORDER BY ID", *decoded_options) == expected
        controls += 1
        expected = "ID\tName\tKind\tData\n" + "".join(display(parents[parent]) + "\t" + display(child[1]) + "\t" + display(child[2]) + "\t" + display(payloads[parent][i]) + "\n" for parent in parents for i, child in enumerate(children[parent]) if child[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileType AS Kind,Files.FileData AS Data FROM Collections WHERE Files.FileType='txt'", *decoded_options) == expected
        controls += 1
        # Hidden members of the displayed collection filter the same child.
        expected = "ID\tName\tData\n" + "".join(display(parents[parent]) + "\t" + display(child[1]) + "\t" + display(payloads[parent][i]) + "\n" for parent in parents for i, child in enumerate(children[parent]) if child[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileData AS Data FROM Collections WHERE Files.FileType='txt'", *decoded_options) == expected
        controls += 1
        expected = "ID\tName\n" + "".join(display(parents[parent]) + "\t" + display(child[1]) + "\n" for parent in parents for child in children[parent] if child[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name FROM Collections WHERE Files.FileType='txt'", *options) == expected
        controls += 1
        assert run(args.cli, "query", "run", "SELECT Count(Files.FileName) AS N FROM Collections WHERE Files.FileData IS NOT NULL", *decoded_options) == "N\n6\n"
        controls += 1
        expected = "AID\tBID\n" + "".join(display(a) + "\t" + display(b) + "\n" for ap,a in parents.items() for bp,b in parents.items() if any(child[2] == chosen for child in children[ap]) and all(value != "N" for value in payloads[bp]))
        sql = "SELECT A.ID AS AID,B.ID AS BID FROM Collections AS A,Collections AS B WHERE A.Files.FileType='txt' AND B.Files.FileData IS NOT NULL ORDER BY A.ID,B.ID"
        assert run(args.cli, "query", "run", sql, *decoded_options) == expected
        controls += 1
        # Displayed A children retain their order/cardinality while hidden B
        # membership uses complete lists, including the explicit decoded view.
        for decoded_view in [False, True]:
            condition = "B.Files.FileData IS NOT NULL" if decoded_view else "B.Files.FileType='txt'"
            selected_parents = [p for p in parents if (all(value != "N" for value in payloads[p]) if decoded_view else any(child[2] == chosen for child in children[p]))]
            expected = "AID\tName\tBID\n" + "".join(display(a) + "\t" + display(child[1]) + "\t" + display(parents[bp]) + "\n" for ap,a in parents.items() for child in children[ap] or [["N"] * len(names)] for bp in selected_parents)
            sql = "SELECT A.ID AS AID,A.Files.FileName AS Name,B.ID AS BID FROM Collections AS A,Collections AS B WHERE " + condition
            assert run(args.cli, "query", "run", sql, *(decoded_options if decoded_view else options)) == expected
            controls += 1
        expected = "AID\tName\tBID\n" + "".join(display(a) + "\t" + display(child[1]) + "\t" + display(b) + "\n" for ap,a in parents.items() for child in children[ap] for bp,b in parents.items() if child[2] == chosen and all(value != "N" for value in payloads[bp]))
        sql = "SELECT A.ID AS AID,A.Files.FileName AS Name,B.ID AS BID FROM Collections AS A,Collections AS B WHERE A.Files.FileType='txt' AND B.Files.FileData IS NOT NULL"
        assert run(args.cli, "query", "run", sql, *decoded_options) == expected
        controls += 1
        # Scalar-parent and mixed aggregate crosstabs retain full heading
        # domains. Counts derive only from inert getters, never SQL execution.
        identity_headings = list(parents.values())
        matched = [(identity,identity) for parent,identity in parents.items() if any(child[2] == chosen for child in children[parent])]
        sql = "TRANSFORM Count(*) SELECT ID FROM Collections WHERE Files.FileType='txt' GROUP BY ID ORDER BY ID PIVOT ID"
        assert run(args.cli, "query", "run", sql, *options) == count_grid(matched, identity_headings)
        controls += 1
        matched = [(a,b) for a in parents.values() for bp,b in parents.items() if all(value != "N" for value in payloads[bp])]
        sql = "TRANSFORM Count(*) SELECT A.ID AS AID FROM Collections AS A,Collections AS B WHERE B.Files.FileData IS NOT NULL GROUP BY A.ID ORDER BY A.ID PIVOT B.ID"
        assert run(args.cli, "query", "run", sql, *decoded_options) == count_grid(matched, identity_headings, "AID")
        controls += 1
        for fixed in [False, True]:
            headings = list(reversed(identity_headings)) if fixed else identity_headings
            expected = "AID\t" + "\t".join(display(value) for value in headings) + "\n"
            if matched:
                for ap,a in parents.items():
                    cells = [str(len(payloads[ap])) if any(identity == heading and all(value != "N" for value in payloads[parent]) for parent,identity in parents.items()) else "NULL" for heading in headings]
                    expected += display(a) + "\t" + "\t".join(cells) + "\n"
            sql = "TRANSFORM Count(A.Files.FileData) SELECT A.ID AS AID FROM Collections AS A,Collections AS B WHERE B.Files.FileData IS NOT NULL GROUP BY A.ID ORDER BY A.ID PIVOT B.ID"
            if fixed:
                sql += " IN (" + ",".join(display(value) for value in headings) + ")"
            assert run(args.cli, "query", "run", sql, *decoded_options) == expected
            controls += 1
        expected = "ID\n" + "".join(display(identity) + "\n" for parent,identity in parents.items() if any(child[2] == chosen for child in children[parent]))
        sql = "SELECT ID FROM Collections WHERE EXISTS(SELECT 1 WHERE Files.FileType='txt') ORDER BY ID"
        assert run(args.cli, "query", "run", sql, *options) == expected
        controls += 1
        expected = "AID\tBID\n" + "".join(display(a) + "\t" + display(b) + "\n" for ap,a in parents.items() for b in parents.values() if any(value != "N" for value in payloads[ap]))
        sql = "SELECT A.ID AS AID,B.ID AS BID FROM Collections AS A,Collections AS B WHERE EXISTS(SELECT 1 WHERE A.Files.FileData IS NOT NULL) ORDER BY A.ID,B.ID"
        assert run(args.cli, "query", "run", sql, *decoded_options) == expected
        controls += 1
        expected = "AID\tName\tBID\n" + "".join(display(a) + "\t" + display(child[1]) + "\t" + display(b) + "\n" for ap,a in parents.items() for child in children[ap] or [["N"] * len(names)] for bp,b in parents.items() if any(value != "N" for value in payloads[bp]))
        sql = "SELECT A.ID AS AID,A.Files.FileName AS Name,B.ID AS BID FROM Collections AS A,Collections AS B WHERE EXISTS(SELECT 1 WHERE B.Files.FileData IS NOT NULL)"
        assert run(args.cli, "query", "run", sql, *decoded_options) == expected
        controls += 1
        # Unprojected ON members are scalar child rows, distinct from parent
        # membership. Compare both outer directions and a hidden WHERE list.
        from_join = " FROM Collections AS A INNER JOIN Collections AS B ON A.ID=B.ID AND A.Files.FileType='txt'"
        inner = [(display(a),display(b)) for ap,a in parents.items() for child in children[ap] or [["N"] * len(names)] for b in parents.values() if a==b and child[2]==chosen]
        def pairs_grid(pairs):
            def number(value):
                return (value != "NULL", int(value) if value != "NULL" else 0)
            return "AID\tBID\n" + "".join(a + "\t" + b + "\n" for a,b in sorted(pairs,key=lambda pair:(number(pair[0]),number(pair[1]))))
        select = "SELECT A.ID AS AID,B.ID AS BID"
        assert run(args.cli,"query","run",select + from_join + " ORDER BY A.ID,B.ID",*options) == pairs_grid(inner)
        controls += 1
        left = [(display(a), display(a) if child[2]==chosen else "NULL") for ap,a in parents.items() for child in children[ap] or [["N"] * len(names)]]
        assert run(args.cli,"query","run",select + from_join.replace("INNER JOIN","LEFT JOIN") + " ORDER BY A.ID,B.ID",*options) == pairs_grid(left)
        controls += 1
        right = inner + [("NULL",display(b)) for b in parents.values() if not any(right==display(b) for _,right in inner)]
        assert run(args.cli,"query","run",select + from_join.replace("INNER JOIN","RIGHT JOIN") + " ORDER BY A.ID,B.ID",*options) == pairs_grid(right)
        controls += 1
        expected = [(display(a),display(a)) for ap,a in parents.items() for index,child in enumerate(children[ap]) if child[2]==chosen and payloads[ap][index]!="N"]
        assert run(args.cli,"query","run",select + from_join + " WHERE A.Files.FileData IS NOT NULL ORDER BY A.ID,B.ID",*decoded_options) == pairs_grid(expected)
        controls += 1
        sql = select + " FROM Collections AS A INNER JOIN Collections AS B ON A.ID=B.ID AND A.Files.FileName IS NOT NULL WHERE B.Files.FileType='txt' ORDER BY A.ID,B.ID"
        expected = [(display(a),display(a)) for ap,a in parents.items() for child in children[ap] if child[1]!="N" and any(value[2]==chosen for value in children[ap])]
        assert run(args.cli,"query","run",sql,*options) == pairs_grid(expected)
        controls += 1
        # Members within Files are zipped; the independent row model never
        # combines a name/type/flag/payload from different stored children.
        conditions = [
            ("EXISTS(SELECT 1 WHERE Files.FileName IS NOT NULL AND Files.FileType='txt')", lambda parent: any(child[1]!="N" and child[2]==chosen for child in children[parent]), False),
            ("EXISTS(SELECT 1 WHERE Files.FileData IS NOT NULL AND Files.FileType='txt')", lambda parent: any(payloads[parent][i]!="N" and child[2]==chosen for i,child in enumerate(children[parent])), True),
            ("EXISTS(SELECT 1 WHERE Files.FileFlags>=0 AND Files.FileType='txt')", lambda parent: any(child[4]!="N" and int(display(child[4]))>=0 and child[2]==chosen for child in children[parent]), False),
            ("IIf(Files.FileName IS NULL,Files.FileFlags IS NULL,False)", lambda parent: any(child[1]=="N" and child[4]=="N" for child in children[parent]), False),
        ]
        for condition, model, data in conditions:
            expected = "ID\n" + "".join(display(identity) + "\n" for parent,identity in parents.items() if model(parent))
            sql = "SELECT ID FROM Collections WHERE " + condition + " ORDER BY ID"
            assert run(args.cli,"query","run",sql,*(decoded_options if data else options)) == expected
            controls += 1
        for negative in [False,True]:
            pairs = []
            for ap,a in parents.items():
                for bp,b in parents.items():
                    hit = any(x[1]!="N" and x[1]==y[1] for x in children[ap] for y in children[bp])
                    unknown = bool(children[ap] and children[bp]) and any(x[1]=="N" or y[1]=="N" for x in children[ap] for y in children[bp])
                    if (not negative and hit) or (negative and not hit and not unknown):
                        pairs.append((display(a),display(b)))
            sql = select + " FROM Collections AS A,Collections AS B WHERE A.Files.FileName" + ("<>" if negative else "=") + "B.Files.FileName ORDER BY A.ID,B.ID"
            assert run(args.cli,"query","run",sql,*options) == pairs_grid(pairs)
            controls += 1
        # Corrupt the final original wrapper/checksum without changing storage
        # framing. Metadata remains readable; completed decoding must fail even
        # if TOP/filter would omit that child. Mutate only a disposable copy.
        metadata = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "AttachmentFixture.java", "--metadata", path)
        encoded = bytes.fromhex(metadata.splitlines()[-1].split("\t")[5][1:])
        original = path.read_bytes()
        assert original.count(encoded) == 1
        changed = bytearray(original)
        changed[original.index(encoded) + len(encoded) - 1] ^= 1
        with tempfile.TemporaryDirectory(prefix="tablecraft-query-attachments-") as directory:
            bad = Path(directory) / "bad.accdb"
            bad.write_bytes(changed)
            # Corrupted wrapper checksums do not change the opaque raw carrier.
            raw_options = ["--database", bad, "--collections", "--native-bytes", "67108864", "--collation", args.collation]
            sql = "SELECT Files.FileData AS Data FROM Collections"
            expected = "Data\n"
            for parent in parents:
                for value in encoded_payloads[parent] or ["N"]:
                    raw_value = bytes.fromhex(value[1:]) if value != "N" else None
                    if raw_value == encoded:
                        raw_value = encoded[:-1] + bytes([encoded[-1] ^ 1])
                    expected += display("N" if raw_value is None else "X" + raw_value.hex()) + "\n"
            assert run(args.cli, "query", "run", sql, *raw_options) == expected
            controls += 1
            bad_options = ["--database", bad, "--collections", "--decoded-attachments", "--native-bytes", "67108864"]
            for sql in ["SELECT Files.FileData FROM Collections", "SELECT TOP 1 Files.FileData FROM Collections", "SELECT Files.FileData FROM Collections WHERE ID=999", "SELECT TOP 1 Files.FileName FROM Collections WHERE Files.FileData IS NOT NULL", "SELECT TOP 1 A.ID FROM Collections AS A,Collections AS B WHERE A.Files.FileData IS NULL", "SELECT TOP 1 A.Files.FileName FROM Collections AS A,Collections AS B WHERE False AND B.Files.FileData IS NULL", "TRANSFORM Count(*) SELECT TOP 1 ID FROM Collections WHERE Files.FileData IS NULL GROUP BY ID PIVOT ID", "SELECT TOP 1 ID FROM Collections WHERE EXISTS(SELECT 1 WHERE Files.FileData IS NOT NULL)", "SELECT TOP 1 A.ID FROM Collections AS A INNER JOIN Collections AS B ON False AND A.Files.FileData IS NOT NULL", "SELECT TOP 1 ID FROM Collections WHERE EXISTS(SELECT 1 WHERE Files.FileData IS NOT NULL AND Files.FileFlags>=0)"]:
                rejected(args.cli, sql, bad_options)
                controls += 1
            assert run(args.cli, "query", "run", "SELECT Count(Files.FileName) AS N FROM Collections", *bad_options) == "N\n6\n"
            controls += 1
            assert bad.read_bytes() == changed
        for sql in ["SELECT Files FROM Collections", "SELECT * FROM Collections"]:
            rejected(args.cli, sql, options)
            controls += 1
        assert digest(path) == before
        assert digest(args.cli) == cli_before, "CLI changed during replay"
    print("Unchanged source", before)
    print(f"Passed {controls} attachment query model controls with {args.collation} policy")


if __name__ == "__main__":
    main()
