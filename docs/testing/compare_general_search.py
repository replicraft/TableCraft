"""Compare General search/replacement with exhaustive independent short-key spans.

No native Access SQL executes. The model deliberately records shortest nonempty
matching spans; all inputs fit complete raw keys, not physical digested prefixes.
"""
import argparse
import os
from pathlib import Path
import tempfile

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def literal(hex_value):
    value = bytes.fromhex(hex_value).decode("utf-8")
    # A NUL cannot enter an OS argv string. It remains a real Text unit in SQL.
    return " & Chr(0) & ".join("'" + part.replace("'", "''") + "'" for part in value.split("\0"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    args = parser.parse_args()
    before = digest(args.jar)
    if before != JACKCESS_SHA256:
        raise ValueError("unexpected independent reader artifact")
    with tempfile.TemporaryDirectory(prefix="tablecraft-general-search-oracle-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, RECIPES / "GeneralSearchOracle.java")
        controls = compare(args, os.pathsep.join([classes, str(args.jar)]))
    if digest(args.jar) != before:
        raise ValueError("independent artifact changed")
    print(f"Passed {controls} General search/replacement controls; native Windows comparisons deferred.")


def compare(args, classpath):
    controls = 0
    for version, policy in [(0, "general-legacy"), (1, "general-2010")]:
        output = run("java", "-Dstdout.encoding=UTF-8", "-cp", classpath,
                     "com.healthmarketscience.jackcess.impl.GeneralSearchOracle", version)
        for row in output.splitlines():
            kind, ident, text_hex, needle_hex, option, expected = row.split("\t")
            text, needle = literal(text_hex), literal(needle_hex)
            if kind == "P":
                expression = f"Replace({text},{needle},'!',1,{option},2)"
                expected = display("T" + expected)
            elif kind == "F":
                expression = f"InStr({option},{text},{needle},2)"
            elif kind == "R":
                expression = f"InStrRev({text},{needle},{option},2)"
            else:
                raise ValueError("unexpected independent result kind")
            actual = run(args.cli,"query","run", "SELECT " + expression + " AS R", "--collation", policy)
            if actual != "R\n" + expected + "\n":
                raise ValueError(f"{policy} case {ident} {kind} {option}: independent result differs")
            controls += 1
        print(f"{policy}: completed {controls} cumulative independent span/function controls",flush=True)
    return controls


if __name__ == "__main__":
    main()
