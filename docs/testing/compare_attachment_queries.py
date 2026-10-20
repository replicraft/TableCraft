"""Compare aligned attachment queries to inert independent stored records.

Uses the original AttachmentFixture recipe and Jackcess reader; this is an
explicit flattening/membership model, not a Microsoft Access SQL oracle.
"""
import argparse
from pathlib import Path

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
        parents, children = {}, {}
        for line in tagged.splitlines():
            cells = line.split("\t")
            if cells[0] == "P":
                assert cells[1] not in parents
                parents[cells[1]] = cells[2]
                children[cells[1]] = []
            else:
                assert len(cells) == 8 and cells[0] in parents
                children[cells[0]].append([cells[2], cells[3], cells[4], cells[6], cells[7]])
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
        for sql in ["SELECT Files.FileData FROM Collections", "SELECT Files FROM Collections", "SELECT * FROM Collections"]:
            rejected(args.cli, sql, options)
            controls += 1
        assert digest(path) == before
        print("Unchanged source", before)
    print(f"Passed {controls} attachment query model controls")


if __name__ == "__main__":
    main()
