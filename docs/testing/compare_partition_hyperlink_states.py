"""Independent Partition/HyperlinkPart admission controls; native Access unrun."""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_partition import label
from compare_union_types import JAR_SHA256


def text(value):
    return display("T" + value.encode().hex())


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def component(link, selector):
    fields = link.split("#", 3)
    fields.extend([""] * (4 - len(fields)))
    shown, address, subaddress, tip = fields
    full = address + ("#" + subaddress if subaddress else "")
    return [shown or address or subaddress, shown, address, subaddress, tip, full][selector]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    controls = 0

    def query(mode, sql, expected=None, options=(), diagnostic=None, policy="en-US"):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", mode, sql, "--number-format", policy, *map(str, options)],
            capture_output=True, text=True, encoding="utf-8",
        )
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, "non-atomic refusal")
            assert diagnostic in result.stderr, (sql, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows[1:], expected)
        controls += 1

    for mode in ["eval", "run"]:
        for code in [0, 1, 11, 255, 65535]:
            error = f"CVErr({code})"
            for arguments in [f"{error},NULL,9,2", f"NULL,{error},9,2", f"1,0,NULL,{error}"]:
                query(mode, f"SELECT Partition({arguments})", [["NULL"]])
            for expression, diagnostic in [
                (f"Partition(1,-1,9,{error})", "type mismatch"),
                (f"Partition({error},0,9,2)", "type mismatch"),
                (f"Partition(NULL,{error},9,1/0)", "division by zero"),
                (f"HyperlinkPart({error},6)", "domain"),
                (f"HyperlinkPart(NULL,{error})", "type mismatch"),
                (f"HyperlinkPart({error},1/0)", "division by zero"),
            ]:
                query(mode, "SELECT 1," + expression, diagnostic=diagnostic)
            query(mode, f"SELECT Partition(CLng({error}),0,9,2)", [[text(label([code, 0, 9, 2]))]])
            query(mode, f"SELECT HyperlinkPart(CStr({error}),0)", [[text(f"Error {code}")]])

        for policy, separator in [("en-US", "."), ("de-DE", ",")]:
            for link in ["", "show#u#s#tip", "#u#s#tip#extra", "##anchor", "é😀#uri#節#tip"]:
                for value in ["0", "1.5", "2.5", "3.5", "5"]:
                    selector = int(Decimal(value).to_integral_value(rounding=ROUND_HALF_EVEN))
                    token = value.replace(".", separator)
                    query(mode, f"SELECT HyperlinkPart('{link}','{token}')", [[text(component(link, selector))]], policy=policy)
            query(mode, "SELECT HyperlinkPart(NULL,'2')", [["NULL"]], policy=policy)
            query(mode, "SELECT 1,HyperlinkPart(NULL,'bad')", diagnostic="type mismatch", policy=policy)

    scalar_controls = controls
    for suffix, original_digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        before = registered_input_hash(path, original_digest)
        assert digest(path) == before
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows",
        ], text=True, encoding="utf-8")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in reference.splitlines()]
        assert len(identities) == len(set(identities)) == 20_000 and 1 in identities
        options = [
            "--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
            "--native-steps", "10000000", "--execution-steps", "10000000",
        ]
        for index in [[], ["--index", "IndexedRows", "IdAsc"]]:
            chosen = [*options, *index]
            suffix_sql = " FROM IndexedRows WHERE ID=1"
            query("run", "SELECT Partition(NULL,CVErr(ID),9,2)" + suffix_sql, [["NULL"]], chosen)
            query("run", "SELECT Partition(CLng(CVErr(ID)),0,9,2)" + suffix_sql, [[text(label([1, 0, 9, 2]))]], chosen)
            query("run", "SELECT HyperlinkPart(CStr(ID),'2.5')" + suffix_sql, [[text("")]], chosen)
            for expression, diagnostic in [
                ("Partition(ID,-1,9,CVErr(ID))", "type mismatch"),
                ("Partition(NULL,CVErr(ID),9,1/(ID-1))", "division by zero"),
                ("HyperlinkPart(CVErr(ID),6)", "domain"),
                ("HyperlinkPart(NULL,CVErr(ID))", "type mismatch"),
                ("HyperlinkPart(CVErr(ID),1/(ID-1))", "division by zero"),
            ]:
                query("run", "SELECT 1," + expression + suffix_sql, options=chosen, diagnostic=diagnostic)
        assert digest(path) == before, "Input changed"
        print(f"{path.name}: unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} controls ({scalar_controls} scalar, {controls - scalar_controls} native); native Access unrun.")


if __name__ == "__main__":
    main()
