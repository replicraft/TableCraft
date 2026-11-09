"""Compare progressive CLI TSV with independent stored rows beyond snapshot/output caps."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES, display
from compare_saved_forests import PROFILES

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-stream-oracle-") as classes:
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes,
            str(RECIPES / "StreamRowsFixture.java"), str(RECIPES / "TableDataOracle.java")], check=True)

        def java(name, *values):
            return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + classes, name, *map(str, values)], text=True, encoding="utf-8")

        def cli(path, *options, accepted=True):
            result = subprocess.run([str(args.cli), "rows", str(path), "StreamRows", *map(str, options)],
                capture_output=True, text=True, encoding="utf-8")
            assert result.returncode == (0 if accepted else 2), result.stderr
            if accepted:
                assert not result.stderr
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("StreamRowsFixture", args.directory / ("stream-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("stream-" + suffix)
            original = path.read_bytes()
            expected = [["ID", "Label"]]
            for line in java("TableDataOracle", path, "StreamRows").splitlines():
                table, identifier, text = line.split("\t")
                assert table == "StreamRows"
                expected.append([display(identifier), display(text)])
            assert len(expected) == 20001
            assert [int(row[0]) for row in expected[1:]] == list(range(1, 20001))
            for batch in (1, 127, 1000):
                output = cli(path, "--stream", "--limit", batch, "--native-rows", 100000,
                    "--native-bytes", 67108864, "--native-steps", 10000000)
                assert len(output.encode()) > 1048576
                actual = list(csv.reader(StringIO(output), delimiter="\t", strict=True))
                assert actual == expected, (path.name, batch)
                controls += 1
            output = cli(path, "--stream", "--column", "ID", "--limit", 127,
                "--native-rows", 100000, "--native-bytes", 67108864, "--native-steps", 10000000)
            assert list(csv.reader(StringIO(output), delimiter="\t", strict=True)) == [[row[0]] for row in expected]
            controls += 1
            # A raised byte allowance is not permission to refill the default row quota.
            partial = cli(path, "--stream", "--limit", 127, "--native-bytes", 67108864,
                "--native-steps", 10000000, accepted=False)
            actual = list(csv.reader(StringIO(partial), delimiter="\t", strict=True))
            assert 1 < len(actual) < len(expected) and actual == expected[:len(actual)]
            controls += 1
            assert path.read_bytes() == original
            print(path.name, len(original), hashlib.sha256(original).hexdigest())
    print(f"Passed {controls} independent 20,000-row stream/projection/quota controls; native Access unrun.")


if __name__ == "__main__":
    main()
