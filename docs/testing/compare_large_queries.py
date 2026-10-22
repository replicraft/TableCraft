"""Compare opt-in larger scalar snapshots/query work with independent stored ID models."""
import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "jet4.mdb": "fbe84bf1000a9e6f9df283c100d2bd1c8e01ab3fab24c50922ccbeb5d722033b",
    "2007.accdb": "397af200f2b08423e99dfc0bfc6487569cc2b8758817d322d398e205e0f2eb05",
    "2016.accdb": "21fe4c647c0768492efb6be460a52e0b3fb90e62ae46c05a0bbd4b90d4a426db",
    "2019.accdb": "894e84373184d7b9740cd093b0df743f4526ba03709c46c2ac6ff687b092b975",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0
    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("stream-" + suffix)
        original = path.read_bytes()
        assert hashlib.sha256(original).hexdigest() == digest
        rows = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "StreamRows"], text=True, encoding="utf-8")
        identifiers = [int(line.split("\t")[1].removeprefix("I32:")) for line in rows.splitlines()]
        assert identifiers == list(range(1, 20001))
        native = ["--native-bytes", "67108864", "--native-rows", "100000", "--native-steps", "10000000"]
        execution = ["--execution-steps", "10000000"]

        def cli(sql, options, accepted=True, reason=None):
            result = subprocess.run([str(args.cli), "query", "run", sql, "--database", str(path), *options],
                text=True, encoding="utf-8", capture_output=True)
            assert result.returncode == (0 if accepted else 2), (path, sql, result.stderr)
            if not accepted:
                assert not result.stdout, (path, sql, result.stdout)
                if reason:
                    assert reason in result.stderr, result.stderr
                return None
            assert not result.stderr
            return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))

        cases = [
            ("SELECT Count(*) AS Total,Max(ID) AS Largest,Min(ID) AS Smallest FROM StreamRows",
             [["Total", "Largest", "Smallest"], [str(len(identifiers)), str(max(identifiers)), str(min(identifiers))]]),
            ("SELECT TOP 3 ID FROM StreamRows ORDER BY ID DESC", [["ID"], *[[str(value)] for value in sorted(identifiers, reverse=True)[:3]]]),
            ("SELECT ID FROM StreamRows WHERE ID IN(1,10000,20000) ORDER BY ID", [["ID"], *[[str(value)] for value in identifiers if value in (1, 10000, 20000)]]),
            ("SELECT ID Mod 2 AS Parity,Count(*) AS Total FROM StreamRows GROUP BY ID Mod 2 ORDER BY Parity",
             [["Parity", "Total"], *[[str(key), str(sum(value % 2 == key for value in identifiers))] for key in (0, 1)]]),
            ("SELECT (SELECT Count(*) FROM StreamRows) AS Total,ID FROM StreamRows WHERE ID=20000", [["Total", "ID"], [str(len(identifiers)), str(max(identifiers))]]),
            ("SELECT ID FROM StreamRows WHERE ID<=2 UNION ALL SELECT ID FROM StreamRows WHERE ID>=19999 ORDER BY ID",
             [["ID"], *[[str(value)] for value in identifiers if value <= 2 or value >= 19999]]),
        ]
        for sql, expected in cases:
            assert cli(sql, native + execution) == expected, (path, sql)
            controls += 1
        cli(cases[0][0], ["--native-bytes", "67108864", "--native-steps", "10000000"], accepted=False)
        controls += 1
        cli(cases[1][0], native, accepted=False, reason="Steps resource limit exceeded")
        controls += 1
        cli(cases[0][0], ["--native-bytes", "67108864", "--native-rows", "20000", "--native-steps", "10000000", *execution], accepted=False, reason="Rows resource limit exceeded")
        controls += 1
        for sql in ("SELECT * FROM StreamRows", "SELECT TOP 1 A.ID FROM StreamRows AS A,StreamRows AS B"):
            cli(sql, native + execution, accepted=False)
            controls += 1
        assert path.read_bytes() == original
        print(path.name, digest, flush=True)
    print(f"Passed {controls} independent 20,000-row aggregate/order/group/subquery/set/limit controls; native Access unrun.")


if __name__ == "__main__":
    main()
