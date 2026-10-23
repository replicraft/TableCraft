"""Independent larger multivalue/attachment/history pages with buffered refusals."""

import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES, escaped
from compare_collection_pages import cell
from compare_large_collections import INPUT_HASHES as COLLECTION_HASHES
from compare_large_stored_objects import INPUT_HASHES as OBJECT_HASHES
from compare_union_types import JAR_SHA256


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "collections-directory", "objects-directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    native = ["--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "100000000"]
    lower = ["--native-bytes", "268435456", "--native-rows", "16384", "--native-steps", "100000000"]
    controls = 0

    def command(path, options, expected=None, accepted=True, diagnostic=None):
        nonlocal controls
        result = subprocess.run([str(args.cli), "rows", str(path), *map(str, options)],
                                text=True, encoding="utf-8", capture_output=True)
        if accepted:
            assert result.returncode == 0 and not result.stderr, (path, options, result.stderr)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert actual == expected, (path, options, actual, expected)
        else:
            assert result.returncode == 2 and not result.stdout, (path, options, result)
            if diagnostic:
                assert diagnostic in result.stderr, (path, options, result.stderr)
        controls += 1

    with tempfile.TemporaryDirectory(prefix="tablecraft-large-pages-reference-") as directory:
        subprocess.run(["javac", "-cp", str(args.jar), "-d", directory,
                        str(RECIPES / "MultivalueOracle.java"), str(RECIPES / "CollectionPageOracle.java")], check=True)
        classpath = str(args.jar) + os.pathsep + directory
        for profile in ["V2016", "V2007"]:
            cases = [
                (args.collections_directory / f"collections-{profile}.accdb", COLLECTION_HASHES[profile],
                 [("ManyParents", 20_000), ("ManyChildren", 400)]),
                (args.objects_directory / f"stored-{profile}.accdb", OBJECT_HASHES[profile],
                 [("ManyFiles", 201), ("Histories", 201)]),
            ]
            for path, registered, tables in cases:
                assert digest(path) == registered, (path, "unregistered input")
                first_pages = []
                for table, count in tables:
                    reference = subprocess.check_output([
                        "java", "-Dstdout.encoding=UTF-8", "-cp", classpath, "CollectionPageOracle", str(path), table,
                    ], text=True, encoding="utf-8")
                    declarations = []
                    records = {}
                    for line in reference.splitlines():
                        parts = line.split("\t")
                        if parts[0] == "H":
                            declarations.append(parts[1:])
                        else:
                            records[int(parts[0]), parts[1]] = parts[2:]
                    assert len(records) == count * len(declarations)
                    headers = []
                    for name, kind in declarations:
                        suffix = {"M": " [multivalue]", "A": " [attachments]"}.get(kind, "")
                        if kind.startswith("H:"):
                            suffix = " [history: " + escaped(kind[2:]) + "]"
                        headers.append(escaped(name) + suffix)

                    def expected(offset, limit):
                        rows = [[cell(kind, records[index, name]) for name, kind in declarations]
                                for index in range(offset, min(offset + limit, count))]
                        return [headers, *rows]

                    modes = ["--collections", "--multivalues"] if table in ["ManyParents", "ManyChildren"] else ["--collections"]
                    for mode in modes:
                        for offset, limit in [(0, 1), (1, 2), (count - 1, 2), (count, 1), (4_294_967_295, 1)]:
                            command(path, [table, mode, "--offset", offset, "--limit", limit, *native], expected(offset, limit))
                        command(path, [table, mode, "--limit", "1"], accepted=False)
                    command(path, [table, "--collections", "--limit", "1", *lower], accepted=False)
                    first_pages.append((table, expected(0, 1)))

                if tables[0][0] == "ManyParents":
                    # The later 101-value table must not publish earlier valid sections.
                    command(path, ["--all", "--collections", "--limit", "1", *native], accepted=False)
                    command(path, ["--all", "--multivalues", "--limit", "1", *native], accepted=False)
                else:
                    all_rows = []
                    for table, rows in first_pages:
                        if all_rows:
                            all_rows.append([])
                        all_rows.extend([["Table", table], *rows])
                    command(path, ["--all", "--collections", "--limit", "1", *native], all_rows)
                    command(path, ["ManyFiles", "--collections", "--limit", "100", *native], accepted=False, diagnostic="output limit exceeded")
                assert digest(path) == registered, (path, "input changed")
                print(path.name, registered, "unchanged", flush=True)
    print(f"Passed {controls} larger collection-page controls; native Access unrun.")


if __name__ == "__main__":
    main()
