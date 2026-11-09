"""Compare inert database/column sort descriptors; do not evaluate native text semantics."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "crypt-jar", "bc-jar", "cli", "database", "table"]:
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--password")
    args = parser.parse_args()
    before = hashlib.sha256(Path(args.database).read_bytes()).digest()
    with tempfile.TemporaryDirectory(prefix="tablecraft-sort-order-") as temporary:
        classpath = os.pathsep.join([args.jar, args.crypt_jar, args.bc_jar, temporary])
        source = Path(__file__).with_name("SortOrderOracle.java")
        subprocess.run(["javac", "-cp", classpath, "-d", temporary, str(source)], check=True, capture_output=True)
        reference = subprocess.run(["java", "-cp", classpath, "SortOrderOracle", args.database, args.table], input=b"" if args.password is None else args.password.encode() + b"\n", check=True, capture_output=True).stdout.decode().splitlines()
        def inspect(*options):
            command = [args.cli, "--format", "tsv", "inspect", args.database, *options]
            if args.password is not None:
                command.extend(["-p", args.password])
            result = subprocess.run(command, check=True, capture_output=True)
            assert not result.stderr
            return list(csv.reader(io.StringIO(result.stdout.decode()), delimiter="\t"))
        root = dict(inspect())
        assert reference[0].split("\t")[1:] == [root["Text sort LCID"], str(int(root["Text sort variant"], 16)), root["Text sort version"]]
        columns = inspect("--table", args.table)
        wanted = [line.split("\t") for line in reference[1:]]
        section = next((i for i, row in enumerate(columns) if row[:1] == ["Text column"]), None)
        actual = [] if section is None else columns[section + 1:]
        assert len(actual) == len(wanted)
        for expected, found in zip(wanted, actual):
            assert bytes.fromhex(expected[1]).decode() == found[0]
            assert expected[2:] == [found[1], str(int(found[3], 16)), found[2]]
    assert hashlib.sha256(Path(args.database).read_bytes()).digest() == before
    print(f"Database and {len(wanted)} text-column descriptors agree; unchanged source")


if __name__ == "__main__":
    main()
