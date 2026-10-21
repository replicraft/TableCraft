"""Check borrowed HyperlinkPart components and optional original native fields.

Jackcess constructs and independently decodes stored fields without executing
the function. Native Microsoft Access comparison remains unrun.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
NATIVE_LINKS = [
    "Microsoft#https://www.microsoft.com#",
    "Customers#https://www.microsoft.com#Form Customers",
    "##Form Customers#Enter Information",
    "#https://example.invalid/#",
    "名🦀#résumé#中#tip#tail",
    None,
    "",
    "label",
]


def components(text):
    if text is None:
        return [None] * 6
    fields = text.split("#", 3)
    fields += [""] * (4 - len(fields))
    display, address, sub, tip = fields
    shown = next((value for value in [display, address, sub] if value), "")
    full = address if not sub else address + "#" + sub
    return [shown, display, address, sub, tip, full]


def execute(cli, mode, sql, database=None, succeeds=True):
    command = [str(cli), "query", mode, sql]
    if database is not None:
        command.extend(["--database", str(database)])
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not succeeds:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []

    assert result.returncode == 0, (sql, result.stderr)
    rows = list(csv.reader(StringIO(result.stdout), delimiter="\t"))[1:]
    return [[None if value == "NULL" else json.loads(value) for value in row] for row in rows]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--work-dir", type=Path)
    args = parser.parse_args()
    if bool(args.jar) != bool(args.work_dir):
        parser.error("--jar and --work-dir must be supplied together")

    rng = random.Random(20261021)
    links = [value for value in NATIVE_LINKS if value is not None]
    for _ in range(64):
        fields = ["" if rng.randrange(3) == 0 else f"part{rng.randrange(100)}" for _ in range(4)]
        links.append("#".join(fields))

    count = 0
    for mode in ["eval", "run"]:
        for text in links:
            projections = ",".join(f"HyperlinkPart('{text}',{part}) AS P{part}" for part in range(6))
            rows = execute(args.cli, mode, "SELECT " + projections)
            assert rows == [components(text)], (text, rows)
            count += 6
        for expression in [
            "HyperlinkPart('x')", "HyperlinkPart('x',)", "HyperlinkPart(*)",
            "HyperlinkPart('x',6)", "HyperlinkPart('x',Null)",
            "HyperlinkPart(Null,1/0)", "HyperlinkPart('x',0),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
        args.work_dir.mkdir(parents=True, exist_ok=True)
        for profile, filename in [
            ("V2000", "hyperlink-jet4.mdb"), ("V2007", "hyperlink-2007.accdb"),
            ("V2016", "hyperlink-2016.accdb"), ("V2019", "hyperlink-2019.accdb"),
        ]:
            database = args.work_dir / filename
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "HyperlinkFixture.java", database, profile)
            before = hashlib.sha256(database.read_bytes()).hexdigest()
            decoded = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "Hyperlinks")
            stored = []
            for line in decoded.splitlines():
                cells = line.split("\t")
                stored.append(None if cells[2] == "N" else bytes.fromhex(cells[2].removeprefix("T")).decode("utf-8"))
            assert stored == NATIVE_LINKS, (profile, stored)
            projections = ",".join(f"HyperlinkPart(Link,{part}) AS P{part}" for part in range(6))
            rows = execute(args.cli, "run", "SELECT " + projections + " FROM Hyperlinks ORDER BY ID", database)
            assert rows == [components(text) for text in stored], (profile, rows)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == before
            print(profile, before)
            count += len(stored) * 7

    print(f"Passed {count} independent hyperlink/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
