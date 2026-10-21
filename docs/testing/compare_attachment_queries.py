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

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2007", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256, "Expected reviewed Jackcess artifact"
    controls = 0
    for path in [args.ace2007, args.ace2016, args.ace2019]:
        before = digest(path)
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "AttachmentFixture.java", "--query-metadata", path)
        parents, children, child_ids = {}, {}, {}
        for line in tagged.splitlines():
            cells = line.split("\t")
            if cells[0] == "P":
                assert cells[1] not in parents
                parents[cells[1]] = cells[2]
                children[cells[1]] = []
                child_ids[cells[1]] = []
            else:
                assert len(cells) == 8 and cells[0] in parents
                children[cells[0]].append([cells[2], cells[3], cells[4], cells[6], cells[7]])
                child_ids[cells[0]].append(cells[1])
        assert len(parents) == 2 and sum(map(len, children.values())) == 6
        options = ["--database", path, "--collections", "--native-bytes", "67108864"]
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
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileType AS Kind FROM Collections WHERE Files.FileType='txt'", *options, "--collation", "ordinal") == expected
        controls += 1
        # Hidden member criteria retain each parent at most once.
        expected = "ID\n" + "".join(display(identity) + "\n" for parent, identity in parents.items() if any(row[2] == chosen for row in children[parent]))
        assert run(args.cli, "query", "run", "SELECT ID FROM Collections WHERE Files.FileType='txt' ORDER BY ID", *options, "--collation", "ordinal") == expected
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
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileType AS Kind,Files.FileData AS Data FROM Collections WHERE Files.FileType='txt'", *decoded_options, "--collation", "ordinal") == expected
        controls += 1
        # Hidden members of the displayed collection filter the same child.
        expected = "ID\tName\tData\n" + "".join(display(parents[parent]) + "\t" + display(child[1]) + "\t" + display(payloads[parent][i]) + "\n" for parent in parents for i, child in enumerate(children[parent]) if child[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name,Files.FileData AS Data FROM Collections WHERE Files.FileType='txt'", *decoded_options, "--collation", "ordinal") == expected
        controls += 1
        expected = "ID\tName\n" + "".join(display(parents[parent]) + "\t" + display(child[1]) + "\n" for parent in parents for child in children[parent] if child[2] == chosen)
        assert run(args.cli, "query", "run", "SELECT ID,Files.FileName AS Name FROM Collections WHERE Files.FileType='txt'", *options, "--collation", "ordinal") == expected
        controls += 1
        assert run(args.cli, "query", "run", "SELECT Count(Files.FileName) AS N FROM Collections WHERE Files.FileData IS NOT NULL", *decoded_options) == "N\n6\n"
        controls += 1
        expected = "AID\tBID\n" + "".join(display(a) + "\t" + display(b) + "\n" for ap,a in parents.items() for bp,b in parents.items() if any(child[2] == chosen for child in children[ap]) and all(value != "N" for value in payloads[bp]))
        sql = "SELECT A.ID AS AID,B.ID AS BID FROM Collections AS A,Collections AS B WHERE A.Files.FileType='txt' AND B.Files.FileData IS NOT NULL ORDER BY A.ID,B.ID"
        assert run(args.cli, "query", "run", sql, *decoded_options, "--collation", "ordinal") == expected
        controls += 1
        # Displayed A children retain their order/cardinality while hidden B
        # membership uses complete lists, including the explicit decoded view.
        for decoded_view in [False, True]:
            condition = "B.Files.FileData IS NOT NULL" if decoded_view else "B.Files.FileType='txt'"
            selected_parents = [p for p in parents if (all(value != "N" for value in payloads[p]) if decoded_view else any(child[2] == chosen for child in children[p]))]
            expected = "AID\tName\tBID\n" + "".join(display(a) + "\t" + display(child[1]) + "\t" + display(parents[bp]) + "\n" for ap,a in parents.items() for child in children[ap] or [["N"] * len(names)] for bp in selected_parents)
            sql = "SELECT A.ID AS AID,A.Files.FileName AS Name,B.ID AS BID FROM Collections AS A,Collections AS B WHERE " + condition
            assert run(args.cli, "query", "run", sql, *(decoded_options if decoded_view else options), "--collation", "ordinal") == expected
            controls += 1
        expected = "AID\tName\tBID\n" + "".join(display(a) + "\t" + display(child[1]) + "\t" + display(b) + "\n" for ap,a in parents.items() for child in children[ap] for bp,b in parents.items() if child[2] == chosen and all(value != "N" for value in payloads[bp]))
        sql = "SELECT A.ID AS AID,A.Files.FileName AS Name,B.ID AS BID FROM Collections AS A,Collections AS B WHERE A.Files.FileType='txt' AND B.Files.FileData IS NOT NULL"
        assert run(args.cli, "query", "run", sql, *decoded_options, "--collation", "ordinal") == expected
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
            bad_options = ["--database", bad, "--collections", "--decoded-attachments", "--native-bytes", "67108864"]
            for sql in ["SELECT Files.FileData FROM Collections", "SELECT TOP 1 Files.FileData FROM Collections", "SELECT Files.FileData FROM Collections WHERE ID=999", "SELECT TOP 1 Files.FileName FROM Collections WHERE Files.FileData IS NOT NULL", "SELECT TOP 1 A.ID FROM Collections AS A,Collections AS B WHERE A.Files.FileData IS NULL", "SELECT TOP 1 A.Files.FileName FROM Collections AS A,Collections AS B WHERE False AND B.Files.FileData IS NULL"]:
                rejected(args.cli, sql, bad_options)
                controls += 1
            assert run(args.cli, "query", "run", "SELECT Count(Files.FileName) AS N FROM Collections", *bad_options) == "N\n6\n"
            controls += 1
            assert bad.read_bytes() == changed
        for sql in ["SELECT Files.FileData FROM Collections", "SELECT Files FROM Collections", "SELECT * FROM Collections"]:
            rejected(args.cli, sql, options)
            controls += 1
        assert digest(path) == before
        print("Unchanged source", before)
    print(f"Passed {controls} attachment query model controls")


if __name__ == "__main__":
    main()
