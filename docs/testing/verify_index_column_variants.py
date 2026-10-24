"""Check native metadata variants against independent Jackcess getter output.

The getter observations originate from IndexColumnsFixture.java. Capacity/spill
counts describe the isolated collection constructors, not whole-process mallocs.
This command performs correctness checks only, without reporting timings.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def observed(binary, mode, path, table, iterations=1):
    result = subprocess.run([
        str(binary), mode, "--database", str(path), "--table", table,
        "--iterations", str(iterations),
    ], capture_output=True, text=True, check=False)
    assert result.returncode == 0 and not result.stderr, result.stderr
    return json.loads(result.stdout)


def key_checksum(columns):
    checksum = 0
    for number, descending in columns:
        checksum = (31 * checksum + number + int(descending)) % 2 ** 64
    return checksum


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    controls = 0
    inputs = {}
    binaries = {
        variant: args.directory / f"index-columns-{variant}"
        for variant in ["vec", "arrayvec", "smallvec"]
    }
    binary_hashes = {name: digest(path) for name, path in binaries.items()}
    for profile, extension in [("V2016", "accdb"), ("V2000", "mdb")]:
        path = args.directory / "fixtures" / f"index-columns-{profile}.{extension}"
        before = digest(path)
        oracle = args.directory / "fixtures" / f"{profile}.full-oracle.tsv"
        expected = {}
        for line in oracle.read_text().splitlines():
            table, name, number, data, root, kind, flags, column, descending = line.split("\t")
            index = expected.setdefault(table, {}).setdefault(name, {
                "name": name,
                "number": int(number),
                "data_number": int(data),
                "root": int(root),
                "kind": int(kind),
                "flags": int(flags, 16),
                "columns": [],
            })
            index["columns"].append([int(column.removeprefix("K")), descending == "TRUE"])

        assert len(expected) == 6
        for table, named in expected.items():
            indexes = list(named.values())
            width = len(indexes[0]["columns"])
            expected_checksum = sum(key_checksum(index["columns"]) for index in indexes) % 2 ** 64
            baseline = None
            for variant, binary in binaries.items():
                description = observed(binary, "describe", path, table)
                descriptors = description["indexes"]
                assert [index["name"] for index in descriptors] == list(named)
                for actual, reference in zip(descriptors, indexes, strict=True):
                    assert {key: actual[key] for key in reference} == reference
                if baseline is None:
                    baseline = descriptors
                assert descriptors == baseline, (variant, profile, table)
                inspection = observed(binary, "inspect", path, table)
                assert inspection["checksum"] == expected_checksum
                controls += len(indexes) * width + 1

            for mode in ["vec", "arrayvec", "smallvec"]:
                result = observed(binaries["vec"], mode, path, table, 100)
                assert result["checksum"] == (100 * expected_checksum) % 2 ** 64
                allocations = 0 if mode == "arrayvec" or mode == "smallvec" and width <= 2 else 100 * len(indexes)
                assert result["container_heap_allocations"] == allocations
                assert result["container_heap_bytes"] == allocations * 10 * description["index_column_bytes"]
                controls += 3
        assert digest(path) == before
        inputs[path.name] = before

    assert {name: digest(path) for name, path in binaries.items()} == binary_hashes
    print(json.dumps({
        "controls": controls,
        "input_sha256": inputs,
        "binary_sha256": binary_hashes,
        "native_access": "unrun",
        "allocation_counting": "capacity/spill-derived collection constructors only",
    }, sort_keys=True))


if __name__ == "__main__":
    main()
