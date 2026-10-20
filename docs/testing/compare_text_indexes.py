"""Compare General Short Text index reads with independently executed Jackcess.

Uses the reviewed artifact and original fixture recipe, never native Access.
Generated database files stay local; original source hashes must remain unchanged.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import PROFILES, RECIPES, display, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            for version in [0, 1]:
                run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    RECIPES / "TextIndexFixture.java",
                    args.directory / f"TextIndexes-{profile}-{version}.{suffix}", profile, version)
    for profile, suffix in PROFILES:
        for version in [0, 1]:
            path = args.directory / f"TextIndexes-{profile}-{version}.{suffix}"
            before = digest(path)
            metadata = run(args.cli, "inspect", path, "--table", "TextIndexed", "--indexes")
            image = path.read_bytes()
            for name in ["TextAsc", "TextDesc"]:
                root = next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith(name + "\t"))
                assert image[root * 4096] == 3, (profile, version, name, "multi-page node root")
            for table, index, count in [("TextIndexed", "TextAsc", 1000), ("TextIndexed", "TextDesc", 1000), ("TextUnique", "UniqueText", 33), ("TextEmpty", "EmptyText", 0), ("TextChecked", "CheckedText", 64), ("TextDigestUnique", "DigestUnique", 1)]:
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    RECIPES / "TableDataOracle.java", path, "--index", table, index)
                lines = [line.split("\t") for line in expected.splitlines()]
                assert len(lines) == count, (profile, version, table, "count")
                if table == "TextIndexed":
                    assert any(row[2] == "N" for row in lines) and any(row[2] == "T" for row in lines)
                assert run(args.rows, path, table, "--index", index, "--bytes", 268435456) == expected, (profile, version, index, "exact tagged rows")
                shown = "ID\tLabel\n" + "".join("\t".join(display(tag) for tag in row[1:]) + "\n" for row in lines)
                assert run(args.cli, "query", "run", f"SELECT ID, Label FROM [{table}]", "--database", path, "--index", table, index) == shown, (profile, version, index, "full query")
                projected = "ID\n" + "".join(display(row[1]) + "\n" for row in lines)
                assert run(args.cli, "query", "run", f"SELECT ID FROM [{table}]", "--database", path, "--index", table, index) == projected, (profile, version, index, "projection")
                if table == "TextIndexed" and version == 1:
                    # A cumulative sequential + three-index check exceeds the
                    # fixed CLI step budget on this deliberately expensive corpus.
                    limited = subprocess.run([str(args.cli), "inspect", str(path), "--table", table, "--check"], capture_output=True)
                    assert limited.returncode == 2 and not limited.stdout and b"Steps resource limit" in limited.stderr, (profile, version, "cumulative check limit")
                else:
                    report = run(args.cli, "inspect", path, "--table", table, "--check")
                    if table == "TextDigestUnique" and version == 1:
                        assert "unsupported" in report.lower(), (profile, version, "digest uniqueness stays unqualified")
                    else:
                        assert "unsupported" not in report.lower() and "checked" in report.lower(), (profile, version, index, "check")
            assert run(args.cli, "query", "run", "SELECT ID FROM TextIndexed WHERE ID >= 995 ORDER BY ID", "--database", path, "--index", "TextIndexed", "TextDesc") == "ID\n995\n996\n997\n998\n999\n", (profile, version, "original filter/sort")
            for table, index in [("TextIgnored", "IgnoredText"), ("TextCompound", "CompoundText")]:
                refused = subprocess.run([str(args.cli), "query", "run", f"SELECT ID FROM [{table}]", "--database", str(path), "--index", table, index], capture_output=True)
                assert refused.returncode == 2 and not refused.stdout, (profile, version, index, "explicit refusal")
            assert digest(path) == before, (profile, version, "source changed")
            print(f"{profile} General {version}: tagged/full/projected reads, bounded checks, filter/sort and explicit limits/refusals; unchanged {before}; {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
