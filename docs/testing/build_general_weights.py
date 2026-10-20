"""Normalize reviewed Jackcess General data for static bounded Rust lookup.

Engineering only. Does not download or execute database content. Requires the
reviewed source/binary artifacts; no Java dependency enters ordinary Rust builds.
"""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import subprocess
import tempfile

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
TYPES = {"SIMPLE": 0, "INTERNATIONAL": 1, "UNPRINTABLE": 2,
         "UNPRINTABLE_EXT": 3, "INTERNATIONAL_EXT": 4, "SIGNIFICANT": 5,
         "SURROGATE": 6, "IGNORED": 7, "REPEAT": 8}
UNITS = 65536


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--vectors-output", type=Path)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    with tempfile.TemporaryDirectory(prefix="tablecraft-general-export-") as directory:
        source = Path(__file__).with_name("ExportGeneralWeights.java")
        subprocess.run(["javac", "-cp", str(args.jar), "-d", directory, str(source)], check=True)
        command = ["java", "-cp", os.pathsep.join([str(args.jar), directory]), "com.healthmarketscience.jackcess.impl.ExportGeneralWeights"]
        lines = subprocess.run(command, check=True, capture_output=True).stdout.decode().splitlines()
        vowels = bytes.fromhex(subprocess.run([*command, "--vowels"], check=True, capture_output=True).stdout.decode().strip())
        if args.vectors_output:
            oracle = Path(__file__).with_name("GeneralKeysOracle.java")
            subprocess.run(["javac", "-cp", str(args.jar), "-d", directory, str(oracle)], check=True)
            oracle_command = ["java", "-cp", os.pathsep.join([str(args.jar), directory]), "com.healthmarketscience.jackcess.impl.GeneralKeysOracle"]
            args.vectors_output.mkdir(parents=True, exist_ok=True)
            for name, options in [("vectors.tsv", []), ("bmp.sha256", ["--exhaustive"])]:
                path = args.vectors_output / name
                path.write_bytes(subprocess.run([*oracle_command, *options], check=True, capture_output=True).stdout)
                print(name, path.stat().st_size, hashlib.sha256(path.read_bytes()).hexdigest())
    assert len(lines) == 2 * UNITS and len(vowels) == 256
    args.output.mkdir(parents=True, exist_ok=True)
    for family, name in enumerate(["general-legacy", "general-2010"]):
        offsets, records, interned = [], bytearray(), {}
        for code, line in enumerate(lines[family * UNITS:(family + 1) * UNITS]):
            fields = line.split("\t")
            assert int(fields[0], 16) == code
            payloads = [b"" if field == "-" else bytes.fromhex(field) for field in fields[8:]]
            assert len(payloads) == 4 and max(map(len, payloads)) <= 8
            record = bytes([TYPES[fields[1]], *map(int, fields[2:8]), *map(len, payloads), 0]) + b"".join(payloads)
            assert len(record) >= 12
            if record not in interned:
                interned[record] = len(records)
                records.extend(record)
            offsets.append(interned[record])
        blob = b"TCGW" + struct.pack("<I", 1) + b"".join(struct.pack("<I", offset) for offset in offsets) + records
        path = args.output / (name + ".bin")
        path.write_bytes(blob)
        print(path.name, len(blob), hashlib.sha256(blob).hexdigest())
    (args.output / "kana-vowels.bin").write_bytes(vowels)
    print("kana-vowels.bin", len(vowels), hashlib.sha256(vowels).hexdigest())


if __name__ == "__main__":
    main()
