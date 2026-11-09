"""Compare explicit General query order with complete independent short keys.

Uses the existing original TextIndexFixture corpus and reviewed Jackcess reader.
This is a bounded weight-model comparison, not native Access SQL qualification.
Long/digested physical keys deliberately do not supply expected SQL ordering.
"""
import argparse
import subprocess
from pathlib import Path

from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
MAX_SHORT_PROBE_UNITS = 20


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if digest(args.jar) != JACKCESS_SHA256:
        raise ValueError("unexpected independent reader artifact")
    controls = 0
    for profile, suffix in PROFILES:
        for version, policy in [(0, "general-legacy"), (1, "general-2010")]:
            path = args.directory / f"TextIndexes-{profile}-{version}.{suffix}"
            before = digest(path)
            independent = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                              RECIPES / "TableDataOracle.java", path, "--index", "TextChecked", "CheckedText")
            selected = []
            for line in independent.splitlines():
                table, ident, value = line.split("\t")
                if table != "TextChecked" or not ident.startswith("I32:"):
                    raise ValueError("unexpected independent row")
                if value != "N":
                    if not value.startswith("T"):
                        raise ValueError("unexpected independent text carrier")
                    text = bytes.fromhex(value[1:]).decode("utf-8")
                    if len(text.encode("utf-16-le")) // 2 >= MAX_SHORT_PROBE_UNITS:
                        continue
                selected.append(ident.removeprefix("I32:"))
            expected = "ID\n" + "".join(ident + "\n" for ident in selected)
            sql = f"SELECT ID FROM TextChecked WHERE Label IS NULL OR Len(Label)<{MAX_SHORT_PROBE_UNITS} ORDER BY Label,ID"
            for options in [[], ["--index", "TextChecked", "CheckedText"]]:
                actual = run(args.cli, "query", "run", sql, "--database", path, "--collation", policy, *options)
                if actual != expected:
                    raise ValueError(f"{profile} {policy}: independent query order differs, selected index={bool(options)}")
                controls += 1
            # Independent exact metadata checks native default/field selection.
            # Both supported field families now execute even when the database
            # default differs; it must never override a column's policy.
            database_order, column_order = run(
                "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                RECIPES / "TextSortOrderOracle.java", path, "TextChecked", "Label"
            ).strip().split("\t")
            if column_order != f"1033:0:{version}":
                raise ValueError("unexpected independently read column descriptor")
            if database_order not in {"1033:0:0", "1033:0:1"}:
                raise ValueError("unexpected unsupported independent database descriptor")
            matched_default = database_order == column_order
            for options in [[], ["--index", "TextChecked", "CheckedText"]]:
                command = [str(value) for value in [args.cli, "query", "run", sql,
                           "--database", path, "--collation", "database", *options]]
                result = subprocess.run(command, capture_output=True, encoding="utf-8", check=False)
                if result.returncode != 0 or result.stdout != expected or result.stderr:
                    raise ValueError(f"{profile}: stored field/default policy differs")
                controls += 1
            if digest(path) != before:
                raise ValueError("query comparison changed source")
            controls += 1
            print(f"{profile} {policy}: {len(selected)} short/NULL rows, sequential/index query order; stored database {database_order}, matched_default={matched_default}; unchanged {before}", flush=True)
    print(f"Passed {controls} General query/source controls; native Windows SQL comparisons deferred.")


if __name__ == "__main__":
    main()
