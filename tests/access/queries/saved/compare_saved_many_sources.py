"""Original 32-source saved reconstruction/getter replay; native Access unrun."""

import argparse
import csv
import hashlib
from io import StringIO
import json
import os
from pathlib import Path
import subprocess


PROFILES = ("V2000", "V2007", "V2016", "V2019")
EXPECTED = {
    "Sources17": ("SELECT", 17, 0),
    "Sources32": ("SELECT", 32, 0),
    "Sources33": ("SELECT", 33, 0),
    "Mixed32": ("SELECT", 32, 16),
    "Joins17": ("SELECT", 32, 17),
    "Cross32": ("CROSS_TAB", 32, 0),
    "Cross33": ("CROSS_TAB", 33, 0),
}
ACCEPTED = ("Sources17", "Sources32", "Mixed32", "Cross32")
JAR_SHA = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Report must be fresh")
    if digest(args.jar) != JAR_SHA:
        raise RuntimeError("Unreviewed reference artifact")

    if args.generate:
        args.directory.mkdir(exist_ok=False)
        classes = args.directory / "classes"
        classes.mkdir()
        compilation = [
            "javac", "-cp", str(args.jar), "-d", str(classes),
            "docs/testing/SavedManySourcesFixture.java",
        ]
        subprocess.run(compilation, check=True)
        generated = []
        for profile in PROFILES:
            suffix = ".mdb" if profile == "V2000" else ".accdb"
            path = args.directory / (profile + suffix)
            common = [
                "java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + str(classes), "SavedManySourcesFixture",
            ]
            writer = common + ["write", str(path), profile]
            subprocess.run(writer, check=True)
            original = digest(path)
            getter_command = common + ["read", str(path)]
            getter = subprocess.run(
                getter_command, capture_output=True, text=True, encoding="utf-8", check=True,
            )
            (args.directory / (profile + ".getters.tsv")).write_text(getter.stdout)
            (args.directory / (profile + ".stderr")).write_text(getter.stderr)
            if getter.stderr or digest(path) != original:
                raise RuntimeError("Generation getter changed an original or reported failure")
            generated.append({
                "profile": profile, "path": str(path), "sha256": original,
                "writer": writer, "getter": getter_command,
            })
        (args.directory / "generation.json").write_text(json.dumps({
            "compilation": compilation, "cases": generated,
        }, indent=2) + "\n")

    pins = [
        args.jar,
        args.cli,
        Path(__file__).resolve().relative_to(Path.cwd().resolve()),
        Path("docs/testing/SavedManySourcesFixture.java"),
        Path("crates/backends/tablecraft-access/src/saved/fragments.rs"),
        Path("crates/backends/tablecraft-access/src/saved/relations/forest.rs"),
        Path("crates/query/tablecraft-query/src/profile.rs"),
        args.directory / "generation.json",
        args.directory / "classes/SavedManySourcesFixture.class",
    ]
    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        pins.extend([
            args.directory / (profile + suffix),
            args.directory / (profile + ".getters.tsv"),
        ])
    before = {str(path): digest(path) for path in pins}
    observations = []
    model_cells = 0

    def cli(arguments, error=None):
        result = subprocess.run(
            [str(args.cli), *map(str, arguments)],
            capture_output=True,
            text=True,
            encoding="utf-8",
        )
        if error is None:
            if result.returncode != 0 or result.stderr:
                raise RuntimeError((arguments, result.returncode, result.stderr))
        elif result.returncode != 2 or result.stdout or error not in result.stderr.lower():
            raise RuntimeError((arguments, result.returncode, result.stdout, result.stderr))
        observations.append({
            "arguments": list(map(str, arguments)),
            "exit": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr.encode()).hexdigest(),
            "expected_error": error,
        })
        return result.stdout

    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        path = args.directory / (profile + suffix)
        getter = subprocess.run(
            [
                "java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + str(args.directory / "classes"),
                "SavedManySourcesFixture", "read", str(path),
            ],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=True,
        )
        if getter.stderr or getter.stdout != (args.directory / (profile + ".getters.tsv")).read_text():
            raise RuntimeError("Original getter capture changed")
        metadata = {}
        sql = {}
        items = []
        for line in getter.stdout.splitlines():
            fields = line.split("\t")
            if fields[0] == "ITEM":
                items.append(int(fields[1]))
            elif fields[0] == "QUERY":
                metadata[fields[1]] = (fields[2], int(fields[3]), int(fields[4]))
            elif fields[0] == "SQL":
                sql[fields[1]] = fields[2]
            else:
                raise RuntimeError("Unknown getter record")
        if metadata != EXPECTED or items != [7]:
            raise RuntimeError("Original table or saved metadata changed")

        for name in ACCEPTED:
            expected = cli(["query", "parse", sql[name]])
            actual = cli(["query", "saved", "--database", path, name])
            if actual != expected:
                raise RuntimeError((profile, name, "Independent rendering differs"))
            rendered = cli([
                "--format", "tsv", "query", "run", "--database", path, "--saved", name,
            ])
            rows = list(csv.reader(StringIO(rendered), delimiter="\t"))
            expected_rows = [["Bucket", "7"], ["0", "1"]] if name == "Cross32" else [
                ["Chosen"], [str(items[0])],
            ]
            if rows != expected_rows:
                raise RuntimeError((profile, name, rows, expected_rows))
            model_cells += len(expected_rows[1])

        for name in ("Sources33", "Cross33"):
            cli(["query", "saved", "--database", path, name], error="unsupported")
        cli(["query", "saved", "--database", path, "Joins17"], error="syntax limit")

    after = {str(path): digest(path) for path in pins}
    if before != after:
        raise RuntimeError("Pinned sources, executable or original evidence changed")
    report = {
        "scope": "Original public-builder fragments and getter-only models; native Access unrun",
        "commands": len(observations),
        "reconstruction_comparisons": len(PROFILES) * len(ACCEPTED),
        "executions": len(PROFILES) * len(ACCEPTED),
        "boundary_refusals": len(PROFILES) * 3,
        "modeled_cells": model_cells,
        "hashes": before,
        "unchanged": True,
        "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in (
        "commands", "reconstruction_comparisons", "executions", "boundary_refusals", "modeled_cells",
    )}))


if __name__ == "__main__":
    main()
