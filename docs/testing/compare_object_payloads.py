"""Compare exact inert catalog payload bytes; this does not extract/execute VBA."""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return command_output(args, result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "example", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--original", type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019, *([args.original] if args.original else [])):
        original = path.read_bytes()
        listing = run("java", "-cp", args.jar, "docs/testing/ObjectPayloadOracle.java", path, "--list")
        records = {int(row[0]): row for row in (line.split("\t") for line in listing.splitlines())}
        expected = run("java", "-cp", args.jar, "docs/testing/ObjectPayloadOracle.java", path, "--all")
        by_id = {}
        for line in expected.splitlines():
            native_id, field, value = line.split("\t")
            by_id.setdefault(int(native_id), []).append(field + "\t" + value + "\n")
        if len(records) != len(by_id):
            raise RuntimeError("Independent catalog/payload record coverage differs")
        payload_bytes = 0
        checked = 0
        limited = 0
        normalized = ""
        for native_id, columns in by_id.items():
            result = subprocess.run([str(args.example), str(path), str(native_id)], capture_output=True, text=True)
            if records[native_id][1] == "LargeObject":
                if records[native_id][4] != "65537" or result.returncode == 0 or result.stdout or "StructureLimit" not in result.stderr:
                    raise RuntimeError("Oversized opaque payload was not rejected atomically")
                limited += 1
                continue
            if result.returncode or result.stdout != "".join(columns):
                raise RuntimeError(f"Opaque payload mismatch for native ID {native_id}: {result.stderr}")
            checked += 1
            normalized += str(native_id) + "\n" + result.stdout
            payload_bytes += sum((len(line.split("\t")[1].strip()) - 1) // 2
                for line in columns if line.split("\t")[1].startswith("X"))
        generated = path != args.original
        if generated:
            objects = {row[1]: row for row in records.values()}
            for name, sizes in (("OpaqueObject", ["2304", "12288", "4"]), ("NullObject", ["N", "N", "N"]), ("EmptyObject", ["0", "0", "0"])):
                if objects[name][3:] != sizes:
                    raise RuntimeError("Original opaque-byte recipe contents differ")
            if limited != 1:
                raise RuntimeError("Expected one explicit bounded-payload rejection")
        else:
            modules = [row for row in records.values() if row[2] == "-32761"]
            if len(modules) != 12 or any(row[3:] != ["N", "N", "N"] for row in modules):
                raise RuntimeError("Registered candidate's inert module observation changed")
            if limited:
                raise RuntimeError("Registered candidate did not complete its bounded comparison")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "objects", checked, "limited", limited, "payload bytes", payload_bytes,
            "file", hashlib.sha256(original).hexdigest(), "payloads", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact raw catalog payloads, NULL/empty distinctions, full capture immutability and bounded rejection passed; module bodies elsewhere/native Access saves remain unqualified.")


if __name__ == "__main__":
    main()
