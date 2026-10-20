"""Independently reopen native encryption/password copies and verify rows and decoded SIDs."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile
from compare_encryption_envelopes import CASES as ENVELOPES
from compare_browsing import display


ORIGINALS = {
    "db2007-enc.accdb": (ENVELOPES["db2007-enc.accdb"][0], "Test123", "Table1"),
    "db2013-enc.accdb": (ENVELOPES["db2013-enc.accdb"][0], "1234", "Customers"),
    "db2007-oldenc.accdb": (ENVELOPES["db2007-oldenc.accdb"][0], "Test123", "Table1"),
    "db-nonstandard.accdb": (ENVELOPES["db-nonstandard.accdb"][0], "password", "Table_One"),
    "db-enc.mdb": ("97c5923f1499872cfaf347d268b5860c67df3cb90f167445db8bd58d9778392a", None, "Table1"),
    "pwd-set.mdb": ("9177af1f5c01c66a3d3be9b61a975856285c18ce6f481baa158bf176726519a5", "testpassword", "Table1"),
    "pwd2-set.mdb": ("dd952d938c5f765d40baa4cdbc5f32f9a936eb3847eefc0eccc2b1d81d483d30", "testpassword", "Table2"),
}
NEW_PASSWORD = "new 😀 password"
RECIPES = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "crypt-jar", "bc-jar", "cli", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--derived-directory", type=Path)
    args = parser.parse_args()
    inputs = [(args.directory / name, *values) for name, values in ORIGINALS.items()]
    if args.derived_directory:
        inputs.extend((path, None, "A😀é", "Records") for path in sorted(args.derived_directory.glob("*-*-*.accdb")))
    with tempfile.TemporaryDirectory(prefix="tablecraft-rewrite-comparison-") as temporary:
        temporary = Path(temporary)
        classpath = os.pathsep.join(str(p) for p in [args.jar, args.crypt_jar, args.bc_jar, temporary])
        subprocess.run(["javac", "-cp", classpath, "-d", str(temporary), str(RECIPES / "NativeSecurityOracle.java"), str(RECIPES / "EncryptedPagesOracle.java")], check=True, capture_output=True)

        def oracle(driver, path, password, *extra):
            result = subprocess.run(["java", "-cp", classpath, driver, str(path), *extra], input=b"" if password is None else password.encode() + b"\n", capture_output=True)
            assert result.returncode == 0, (path.name, result.stderr)
            return result.stdout

        def cli(*command, password=None):
            invocation = [str(args.cli), "--format", "tsv", *map(str, command)]
            if password is not None:
                invocation.extend(["-p", password])
            return subprocess.run(invocation, capture_output=True)

        for number, (source, digest, password, table) in enumerate(inputs):
            before = source.read_bytes()
            if digest is not None:
                assert hashlib.sha256(before).hexdigest() == digest, source.name
            records = oracle("EncryptedPagesOracle", source, password, "--rows", table)
            parsed = [line.split("\t") for line in records.decode().splitlines()]
            expected = [parsed[0][1:], *[[display(value) for value in row[1:]] for row in parsed[1:]]]
            sids = oracle("NativeSecurityOracle", source, password)
            for action, output_password in [("remove", None), ("change-password", NEW_PASSWORD)]:
                output = temporary / f"{number}-{action}{source.suffix}"
                command = ["encryption", action, source, "--output", output]
                if action == "change-password":
                    command.extend(["--new-password", NEW_PASSWORD])
                result = cli(*command, password=password)
                assert result.returncode == 0 and not result.stderr, (source.name, action, result.stderr)
                assert oracle("NativeSecurityOracle", output, output_password) == sids, (source.name, action, "SID preservation")
                assert oracle("EncryptedPagesOracle", output, output_password, "--rows", table) == records, (source.name, action, "independent rows")
                query = cli("query", "run", f"SELECT * FROM [{table}]", "--database", output, password=output_password)
                assert query.returncode == 0 and not query.stderr, (source.name, action, query.stderr)
                assert list(csv.reader(io.StringIO(query.stdout.decode()), delimiter="\t")) == expected
                after = output.read_bytes()
                assert after[20:24] == before[20:24], (source.name, "changed native version")
                if action == "change-password":
                    refused = cli("encryption", "remove", output, "--output", temporary / "wrong-password-output", password=password)
                    # An originally unpassworded Jet file now also has a password.
                    assert refused.returncode == 2 and not refused.stdout
                    assert not (temporary / "wrong-password-output").exists()
                    if source.name in ["db2007-enc.accdb", "db2013-enc.accdb"]:
                        assert after[4096:] == before[4096:], "Agile rewrap changed data ciphertext"
                protected = cli(*command, password=password)
                assert protected.returncode == 2 and output.read_bytes() == after, "existing output overwritten"
            assert source.read_bytes() == before, (source.name, "source changed")
            print(f"{source.name}: remove/change reopen; exact rows and decoded SIDs; old-password/output protections; unchanged source", flush=True)


if __name__ == "__main__":
    main()
