"""Compare explicit history query views to independent inert stored records.

This is an original stored-order/collection model, not a native Access SQL oracle.
Original inputs and the CLI binary are verified unchanged after all controls.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from compare_browsing import display, run
from compare_cached_pages import date_display
from compare_version_history import records

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def identifier(name):
    if "]" in name:
        raise ValueError("Recipe identifier contains an unsupported bracket")
    return "[" + name + "]"


def decoded(tag):
    if not tag.startswith("T"):
        raise ValueError("Expected tagged Text declaration")
    return bytes.fromhex(tag[1:]).decode("utf-8")


def refused(cli, sql, options):
    result = subprocess.run(
        list(map(str, [cli, "query", "run", sql, *options])),
        capture_output=True,
        text=True,
    )
    assert result.returncode != 0 and not result.stdout, (sql, result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2007", "ace2016", "ace2019"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--collation", choices=("ordinal", "database"), default="ordinal")
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256, "Expected reviewed reference reader"
    binary = digest(args.cli)
    controls = 0
    inputs = []

    for path in (args.ace2007, args.ace2016, args.ace2019):
        before = digest(path)
        for table, memo in (("Histories", "Notes"), ("OtherHistories", "Details")):
            java = ["java", "-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-cp", args.jar]
            metadata = run(
                *java, "docs/testing/VersionHistoryQueryOracle.java", path, table, memo
            ).splitlines()
            declaration = metadata[0].split("\t")
            assert declaration[0] == "D" and len(declaration) == 4
            hidden, value, modified = map(decoded, declaration[1:])
            parents = [line.split("\t") for line in metadata[1:]]
            tagged = run(
                *java, "docs/testing/VersionHistoryOracle.java", path, table, memo
            ).splitlines()
            assert len(parents) == len(tagged)
            children = [records(line) for line in tagged]
            options = [
                "--database", path, "--collections", "--format", "tsv",
                "--native-bytes", "67108864", "--collation", args.collation,
            ]
            member = identifier(hidden) + "." + identifier(value)
            changed = identifier(hidden) + "." + identifier(modified)
            source = identifier(table)

            for selections in ((0,), (1,), (0, 1), (1, 0)):
                names = ["HistoryText" if position == 0 else "HistoryTime" for position in selections]
                columns = [member if position == 0 else changed for position in selections]
                sql = "SELECT ID," + ",".join(
                    column + " AS " + name for column, name in zip(columns, names)
                ) + " FROM " + source
                expected = "ID\t" + "\t".join(names) + "\n"
                for parent, versions in zip(parents, children):
                    for _, text, time in versions or [("N", "N", "N")]:
                        cells = [display(text), date_display(time)]
                        expected += display(parent[2]) + "\t" + "\t".join(
                            cells[position] for position in selections
                        ) + "\n"
                assert run(args.cli, "query", "run", sql, *options) == expected, (path, sql)
                controls += 1

            # Current Memo values remain independent of older stored versions.
            sql = f"SELECT ID,{identifier(memo)} AS CurrentMemo,{member} AS HistoryText FROM {source}"
            expected = "ID\tCurrentMemo\tHistoryText\n"
            for parent, versions in zip(parents, children):
                for _, text, _ in versions or [("N", "N", "N")]:
                    expected += "\t".join(map(display, (parent[2], parent[3], text))) + "\n"
            assert run(args.cli, "query", "run", sql, *options) == expected
            controls += 1

            sql = f"SELECT ID,Count({member}) AS N FROM {source} GROUP BY ID ORDER BY ID"
            expected = "ID\tN\n" + "".join(
                display(parent[2]) + "\t" + str(sum(text != "N" for _, text, _ in versions)) + "\n"
                for parent, versions in zip(parents, children)
            )
            assert run(args.cli, "query", "run", sql, *options) == expected
            controls += 1

            for present in (False, True):
                sql = f"SELECT ID FROM {source} WHERE {identifier(hidden)} IS "
                sql += "NOT NULL" if present else "NULL"
                sql += " ORDER BY ID"
                expected = "ID\n" + "".join(
                    display(parent[2]) + "\n"
                    for parent, versions in zip(parents, children)
                    if bool(versions) == present
                )
                assert run(args.cli, "query", "run", sql, *options) == expected
                controls += 1

            sql = f"SELECT ID,{member} AS HistoryText,{changed} AS HistoryTime FROM {source} WHERE {member}='same'"
            expected = "ID\tHistoryText\tHistoryTime\n" + "".join(
                display(parent[2]) + "\t" + display(text) + "\t" + date_display(time) + "\n"
                for parent, versions in zip(parents, children)
                for _, text, time in versions
                if text == "T" + "same".encode().hex()
            )
            assert run(args.cli, "query", "run", sql, *options) == expected
            controls += 1

            for sql in (
                f"SELECT {identifier(hidden)} FROM {source}",
                f"SELECT {identifier(hidden)}.FileData FROM {source}",
            ):
                refused(args.cli, sql, options)
                controls += 1
            refused(args.cli, f"SELECT {member} FROM {source}", ["--database", path])
            refused(args.cli, f"SELECT {member} FROM {source}", ["--database", path, "--multivalues"])
            controls += 2

            for extra in (("--native-steps", "1"), ("--execution-steps", "1"), ("--output-bytes", "1")):
                refused(args.cli, f"SELECT {member} FROM {source}", [*options, *extra])
                controls += 1

        assert digest(path) == before, "Original history input changed"
        inputs.append({"file": path.name, "sha256": before})
    assert digest(args.cli) == binary, "CLI binary changed during replay"
    print(json.dumps({
        "query_controls": controls,
        "collation": args.collation,
        "inputs": inputs,
        "cli_sha256": binary,
        "native_access": "unrun",
    }, sort_keys=True))


if __name__ == "__main__":
    main()
