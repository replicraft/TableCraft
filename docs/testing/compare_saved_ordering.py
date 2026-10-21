"""Independent stored-key ordering, raw physical getters and explicit row controls."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES
from compare_saved_queries import catalog_ids
from compare_composite_constraints import roots
from compare_complex_links import column_layout
from compare_attachment_columns import fields
from compare_decimal_relationships import variable_start


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
    expected = {
        "OrderProjection": [["C1", "C2", "C3", "C4"], ["1", "2", "3", "4"]],
        "OrderEmpty": [["C1", "C2"], ["1", "2"]],
        "OrderTies": [["C2", "C1"], ["2", "1"]],
        "OrderNullColumn": [["C1", "C2"], ["1", "2"]],
        "OrderNullOther": [["C1", "C2"], ["1", "2"]],
        "OrderParameters": [["KeyA"], ["2"], ["4"]],
        "OrderSort": [["KeyA", "KeyB"], *[[str(a), str(b)] for b in [4, 3, 2] for a in [1, 2, 4]]],
        "OrderSources": [["KeyA", "KeyB"], *[[str(a), str(b)] for b in [2, 3, 4] for a in [1, 2, 4]]],
        "OrderJoin": [["KeyA", "KeyB", "KeyC"], ["1", "NULL", "NULL"], ["2", "NULL", "NULL"], ["4", "4", "NULL"]],
        "OrderCrosstab": [["Bucket", "Total", "1", "2", "4"], ["0", "3", "1", "1", "1"]],
        "OrderUnion": [["KeyA", "KeyB"], ["2", "1"], ["3", "1"], ["4", "1"], ["1", "0"], ["2", "0"], ["4", "0"]],
    }
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-ordering-oracle-") as classes:
        sources = [RECIPES / (name + ".java") for name in
                   ("SavedForestFixture", "SavedOrderingFixture", "SavedReadOracle", "SavedQueryOracle")]
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes, *map(str, sources)], check=True)

        def java(name, *values):
            return subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + classes, name, *map(str, values)
            ], text=True, encoding="utf-8")

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True)
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("SavedOrderingFixture", args.directory / ("order-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("order-" + suffix)
            before = path.read_bytes()
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 25
            names = [name for name in names if name.startswith("Order")]
            assert set(names) == set(expected)
            for name in names:
                raw = java("SavedQueryOracle", path, name)
                report = cli("inspect", path, "--query", name)
                assert list(csv.reader(StringIO(report), delimiter="\t")) == [line.split("\t") for line in raw.splitlines()], name
                controls += 1
                sql = java("SavedReadOracle", path, name)
                assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                controls += 1
                for query in (sql, None, "SELECT * FROM [" + name + "]"):
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path]
                    if name == "OrderParameters":
                        command += ["--param", "Floor=long:2", "--param", "Ceiling=long:4"]
                    output = cli(*command)
                    assert list(csv.reader(StringIO(output), delimiter="\t")) == expected[name], (name, output)
                    controls += 1
            ids = catalog_ids(args, path)
            definition = ids["MSysQueries"]
            object_offset, _, columns = column_layout(before, definition, "ObjectId")
            layout = fields(before, definition)
            # Duplicate/NULL native keys violate the system primary index. Read
            # them only as deliberately inconsistent copies, never native-save
            # qualification or a reason to modify an original fixture.
            attribute_at = layout["Attribute"][0]
            attribute_offset = int.from_bytes(before[attribute_at+21:attribute_at+23], "little")
            for name in ("OrderTies", "OrderProjection", "OrderNullColumn", "OrderNullOther"):
                slots = [slot for slot in roots(before, definition)
                         if int.from_bytes(before[slot[0]+2+object_offset:slot[0]+6+object_offset], "little", signed=True) == ids[name]]
                changed = bytearray(before)
                for slot in slots:
                    start = variable_start(before, slot, layout["Order"][3], columns)
                    if name == "OrderTies" and before[start:start+1] == b"\x02":
                        changed[start] = 1
                    elif name == "OrderProjection" and before[start:start+1] == b"\x81":
                        changed[start] = 128
                    elif ((name == "OrderNullColumn" and before[slot[0]+2+attribute_offset] == 6 and before[start:start+1] == b"\x01")
                          or (name == "OrderNullOther" and before[slot[0]+2+attribute_offset] == 0)):
                        number = layout["Order"][2]
                        mask = slot[1] - (columns+7)//8
                        changed[mask+number//8] &= ~(1 << (number%8))
                assert changed != before
                with tempfile.TemporaryDirectory(prefix="tablecraft-order-tie-copy-") as directory:
                    copy = Path(directory) / path.name
                    copy.write_bytes(changed)
                    sql = java("SavedReadOracle", copy, name)
                    assert cli("query", "saved", "--database", copy, name) == cli("query", "parse", sql)
                    output = cli("query", "run", "--database", copy, "--saved", name)
                    wanted = [["C2", "C1"], ["2", "1"]] if name.startswith("OrderNull") else expected[name]
                    assert list(csv.reader(StringIO(output), delimiter="\t")) == wanted
                    assert copy.read_bytes() == changed
                    controls += 2
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {controls} native ordering/getter/rendering/execution/read-copy controls; native Access unrun.")


if __name__ == "__main__":
    main()
