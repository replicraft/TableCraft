"""Independently qualify encrypted modern ACE history reads and fresh copies.

Public fixture passwords are test inputs, never user credentials. No Access
expressions execute in the reference reader. Native Windows qualification is unrun.
"""

import argparse
import csv
from io import StringIO
import json
import os
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_modern_scalar_histories import INPUT_HASHES, columns, digest, model
from compare_mixed_scalar_histories import EXECUTION, NATIVE
from encryption_references import rewrite_classpath

PASSWORD = "modern 😀 é fixture"
NEW_PASSWORD = "changed 😀 é fixture"
ENCRYPTED_HASHES = {
    "V2016-reference.accdb": "e0c8f61ed6027d3e226a431dd327af4aa12755dcc830679770b5a921603afebf",
    "V2016-tablecraft.accdb": "a4550ccfb33cbad9505ec2acaeaaeea198afa729e067c1bdc9a4b9797bc0d29b",
    "V2019-reference.accdb": "6de05f7da430bfd3e8dc1cabb8dfa7755890ff2a8582c4e04f233c510f223a4d",
    "V2019-tablecraft.accdb": "a9fbb4f700ac1b0955e1b3a9c8062f9b6e5919bd2d8d9f2b4576b89790282d9e",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "pages", "jar", "crypt-jar", "bc-jar", "directory", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    classpath = rewrite_classpath(args.jar, args.crypt_jar, args.bc_jar)
    if args.generate:
        args.output.mkdir(parents=True, exist_ok=False)
    else:
        assert args.output.is_dir(), "Missing registered encrypted corpus"
    classes = args.output / "classes"
    classes.mkdir(exist_ok=True)
    subprocess.run([
        "javac", "-cp", classpath, "-d", str(classes),
        *[str(RECIPES / (name + ".java")) for name in [
            "TableDataOracle", "ModernEncryptedOracle", "AddEncryptionOracle",
            "NativeSecurityOracle", "RemoveAddedEncryptionOracle", "EncryptedPagesOracle",
        ]],
    ], check=True, capture_output=True)
    classpath = os.pathsep.join([classpath, str(classes)])
    controls = 0
    inventory = []

    def oracle(name, path, password, *extra):
        nonlocal controls
        result = subprocess.run(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", classpath, name, str(path), *map(str, extra)],
            input="" if password is None else password + "\n",
            capture_output=True, text=True, encoding="utf-8",
        )
        assert result.returncode == 0 and not result.stderr, (name, path.name, result.stderr)
        controls += 1
        return result.stdout

    def cli(arguments, password=None, wanted=None, diagnostic=None):
        nonlocal controls
        command = [str(args.cli), "--format", "tsv", *map(str, arguments)]
        if password is not None:
            command.extend(["-p", password])
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (arguments[:2], "non-atomic failure")
            assert diagnostic in result.stderr, (arguments[:2], result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (arguments[:2], result.stderr)
            if wanted is not None:
                rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert rows == wanted, (arguments[:2], "different displayed data")
        controls += 1
        return result.stdout

    for profile in ["V2016", "V2019"]:
        plain = args.directory / f"modern-{profile}.accdb"
        plain_before = digest(plain)
        assert plain_before == INPUT_HASHES[profile], "Unregistered plaintext"
        expected = model(profile == "V2019")
        names = columns(profile == "V2019")
        plain_records = oracle("ModernEncryptedOracle", plain, None, "ModernHistory")
        records = [line.split("\t")[1:] for line in plain_records.splitlines()]
        identities = [int(row[0][4:]) for row in records]
        assert len(identities) == len(set(identities)) == len(expected)
        assert all(row == expected[identity] for identity, row in zip(identities, records))
        displayed = [names, *[[display(tag) for tag in row] for row in records]]
        sids = oracle("NativeSecurityOracle", plain, None)

        for producer in ["reference", "tablecraft"]:
            encrypted = args.output / f"{profile}-{producer}.accdb"
            changed = args.output / f"{profile}-{producer}-changed.accdb"
            removed = args.output / f"{profile}-{producer}-removed.accdb"
            independent = args.output / f"{profile}-{producer}-independent.accdb"
            if args.generate:
                if producer == "reference":
                    oracle("AddEncryptionOracle", plain, PASSWORD, encrypted)
                else:
                    cli(["encryption", "add", plain, "--output", encrypted,
                         "--new-password", PASSWORD])
            else:
                assert digest(encrypted) == ENCRYPTED_HASHES[encrypted.name]
            before = digest(encrypted)
            assert encrypted.read_bytes()[4096:] != plain.read_bytes()[4096:]
            assert oracle("ModernEncryptedOracle", encrypted, PASSWORD, "ModernHistory") == plain_records
            assert oracle("NativeSecurityOracle", encrypted, PASSWORD) == sids

            # Every encrypted data page is independently decrypted by both engines.
            reference_pages = oracle("EncryptedPagesOracle", encrypted, PASSWORD)
            decoded = subprocess.run(
                [str(args.pages), str(encrypted)], input=PASSWORD + "\n",
                capture_output=True, text=True, encoding="utf-8",
            )
            assert decoded.returncode == 0 and not decoded.stderr, decoded.stderr
            assert decoded.stdout == reference_pages, "Decoded page hashes differ"
            assert len(reference_pages.splitlines()) == encrypted.stat().st_size // 4096 - 1
            controls += 1

            report = cli(["encryption", "inspect", encrypted])
            assert "Agile" in report and "Not performed" in report
            cli(["rows", encrypted, "ModernHistory", "--stream", "--limit", "257", *NATIVE],
                password=PASSWORD, wanted=displayed)

            def query(sql, wanted=None, extra=(), diagnostic=None, password=PASSWORD, native=NATIVE):
                return cli(
                    ["query", "run", sql, "--database", encrypted, *native, *EXECUTION, *extra],
                    password=password, wanted=wanted, diagnostic=diagnostic,
                )

            query("SELECT Count(*) AS N,Count(Wide) AS W FROM ModernHistory", [
                ["N", "W"], [str(len(expected)), str(sum(row[1] != "N" for row in records))],
            ])
            last = sorted(expected, reverse=True)[:17]
            query("SELECT TOP 17 * FROM ModernHistory ORDER BY ID DESC", [
                names, *[[display(tag) for tag in expected[identity]] for identity in last],
            ])
            index_records = oracle("ModernEncryptedOracle", encrypted, PASSWORD, "ModernHistory", "WideAsc")
            indexed = [line.split("\t")[1:] for line in index_records.splitlines()]
            index_ids = [int(row[0][4:]) for row in indexed]
            assert len(index_ids) == len(set(index_ids)) == len(expected)
            assert all(row == expected[identity] for identity, row in zip(index_ids, indexed))
            query("SELECT ID,Wide FROM ModernHistory", [
                ["ID", "Wide"], *[[display(row[0]), display(row[1])] for row in indexed],
            ], ["--index", "ModernHistory", "WideAsc"])
            if profile == "V2019":
                position = names.index("Stamp")
                chosen = sorted(
                    expected,
                    key=lambda identity: (expected[identity][position] != "N", expected[identity][position]),
                    reverse=True,
                )[:17]
                query("SELECT TOP 17 ID,Stamp,LaterStamp FROM ModernHistory ORDER BY Stamp DESC,ID", [
                    ["ID", "Stamp", "LaterStamp"], *[
                        [str(identity), display(expected[identity][position]), display(expected[identity][-1])]
                        for identity in chosen
                    ],
                ])
            for supplied in [None, "public wrong password", ""]:
                query("SELECT Count(*) FROM ModernHistory", diagnostic=(
                    "password is required" if supplied is None else "incorrect database password"
                ), password=supplied)
            query(
                "SELECT Count(*) FROM ModernHistory",
                native=["--native-bytes", "1", *NATIVE[2:]],
                diagnostic="limit",
            )

            if args.generate:
                cli(["encryption", "change-password", encrypted, "--output", changed,
                     "--new-password", NEW_PASSWORD], password=PASSWORD)
                cli(["encryption", "remove", encrypted, "--output", removed], password=PASSWORD)
                oracle("RemoveAddedEncryptionOracle", encrypted, PASSWORD, independent)
            assert changed.read_bytes()[4096:] == encrypted.read_bytes()[4096:], "Rewrap changed ciphertext"
            assert oracle("ModernEncryptedOracle", changed, NEW_PASSWORD, "ModernHistory") == plain_records
            assert oracle("NativeSecurityOracle", changed, NEW_PASSWORD) == sids
            cli(["query", "run", "SELECT Count(*) FROM ModernHistory", "--database", changed],
                password=PASSWORD, diagnostic="incorrect database password")
            assert removed.read_bytes() == plain.read_bytes(), "TableCraft removal differs from original image"
            assert independent.read_bytes() == plain.read_bytes(), "Independent removal differs from original image"
            controls += 2
            assert digest(encrypted) == before, "Encrypted input changed"
            inventory.append({"name": encrypted.name, "bytes": encrypted.stat().st_size, "sha256": before})
            print(f"{encrypted.name}: {len(expected)} exact rows; decoded pages/SIDs agree; unchanged {before}", flush=True)
        assert digest(plain) == plain_before, "Plaintext original changed"

    if args.generate:
        (args.output / "inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    print(f"Passed {controls} encrypted modern history controls; native Access unrun.")


if __name__ == "__main__":
    main()
