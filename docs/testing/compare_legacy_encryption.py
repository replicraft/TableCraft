"""Compare qualified legacy native codecs with pinned independent encrypted files."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile
from compare_agile_pages import RECIPES, run
from compare_browsing import display


CASES = {
    "db-enc.mdb": ("97c5923f1499872cfaf347d268b5860c67df3cb90f167445db8bd58d9778392a", None, "Table1"),
    "db2007-oldenc.accdb": ("174ba7ff6349d4f929adf555f5035c0722923776662f30a523f2933827d9558e", "Test123", "Table1"),
    "db-nonstandard.accdb": ("5e93bad848e0262ea8952c14db897d63f8588ca22870043ab7cf954d865b4b39", "password", "Table_One"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "crypt-jar", "bc-jar", "reader", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    classpath = os.pathsep.join(str(p) for p in [args.jar, args.crypt_jar, args.bc_jar])
    for name, (digest, password, table) in CASES.items():
        path = args.directory / name
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        reference = run(["java", "-cp", classpath, RECIPES / "EncryptedPagesOracle.java", path], password)
        actual = run([args.reader, path], password)
        assert reference.returncode == actual.returncode == 0, (name, reference.stderr, actual.stderr)
        assert reference.stdout == actual.stdout, (name, "all decoded pages")
        records = run(["java", "-cp", classpath, RECIPES / "EncryptedPagesOracle.java", path, "--rows", table], password)
        assert records.returncode == 0, (name, records.stderr)
        records = [line.split("\t") for line in records.stdout.decode().splitlines()]
        expected = [records[0][1:], *[[display(v) for v in row[1:]] for row in records[1:]]]

        def cli(*command, credential=password):
            invocation = [str(args.cli), "--format", "tsv", *map(str, command)]
            if credential is not None:
                invocation.extend(["-p", credential])
            return subprocess.run(invocation, capture_output=True, check=False)

        def cells(result):
            assert result.returncode == 0 and not result.stderr, (name, result.stderr)
            return list(csv.reader(io.StringIO(result.stdout.decode()), delimiter="\t", strict=True))

        def browse(source):
            assert cells(cli("rows", source, table)) == expected
            assert cells(cli("query", "run", f"SELECT * FROM [{table}]", "--database", source)) == expected
            assert cells(cli("query", "run", f"SELECT Count(*) AS Total FROM [{table}]", "--database", source)) == [["Total"], [str(len(expected) - 1)]]
            assert [table, "User", str(len(expected) - 1)] in cells(cli("tables", source, "--count"))

        browse(path)
        if password is not None:
            for credential, message in [(None, b"database password is required"), ("wrong-public-password", b"incorrect database password")]:
                failed = cli("rows", path, table, credential=credential)
                assert failed.returncode == 2 and not failed.stdout and message in failed.stderr
                assert b"wrong-public-password" not in failed.stderr
            # Route unchanged cipher/data pages through all qualified ACE families.
            # These are derivatives, not native-authored generation evidence.
            with tempfile.TemporaryDirectory(prefix="tablecraft-legacy-routing-") as directory:
                for code in [2, 3, 5, 6]:
                    copy = bytearray(before)
                    copy[20] = code
                    target = Path(directory) / f"profile-{code}.accdb"
                    target.write_bytes(copy)
                    browse(target)
                    assert target.read_bytes() == copy
        assert path.read_bytes() == before
        print(f"{name}: {len(actual.stdout.splitlines())} exact pages; {len(expected) - 1} rows/query/count; credentials and routing agree; unchanged {digest}")


if __name__ == "__main__":
    main()
