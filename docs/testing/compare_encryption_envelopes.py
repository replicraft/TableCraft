"""Inspect pinned upstream encryption inputs without decrypting or modifying them."""
import argparse
import csv
import hashlib
import io
from pathlib import Path
import subprocess


CASES = {
    "db2007-enc.accdb": ("fa6cd4ed7639dee40ad288ff6c530356ea050bde36dbde5e4328fee6aa07f306", "4.4", "Agile", "911", None),
    "db2007-oldenc.accdb": ("174ba7ff6349d4f929adf555f5035c0722923776662f30a523f2933827d9558e", "4.2", "StandardOrCryptoApi", "190", "0x00000004"),
    "db2013-enc.accdb": ("3e9ee219a7ca9bf7dee261f6455f298887603d41c82ea0c5bc06d8831f09637a", "4.4", "Agile", "1055", None),
    "db-nonstandard.accdb": ("5e93bad848e0262ea8952c14db897d63f8588ca22870043ab7cf954d865b4b39", "4.2", "StandardOrCryptoApi", "224", "0x0000000c"),
}


def run(cli, path, *args):
    return subprocess.run([str(cli), "--format", "tsv", "inspect", str(path), *args], capture_output=True, check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    for name, (digest, version, family, length, flags) in CASES.items():
        path = args.directory / name
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        assert before == digest, (name, "source hash")
        result = run(args.cli, path, "--encryption")
        assert result.returncode == 0 and not result.stderr, (name, result.stderr)
        rows = dict(csv.reader(io.StringIO(result.stdout.decode()), delimiter="\t"))
        expected = {
            "Page encoding": "Office encryption", "Envelope version": version,
            "Envelope family": family, "Descriptor bytes": length,
            "Cipher configuration": "Not validated", "Password verification": "Not implemented",
            "Page decoding": "Not implemented",
        }
        if flags is not None:
            expected["Outer flags"] = flags
        assert rows == expected, (name, rows)
        refused = run(args.cli, path, "--catalog")
        assert refused.returncode == 2 and not refused.stdout, (name, "encoded read gate")
        assert hashlib.sha256(path.read_bytes()).hexdigest() == before, (name, "changed source")
        print(f"{name}: envelope matches pinned reference; encoded reads refused; unchanged {before}")


if __name__ == "__main__":
    main()
