"""Compare conservative Short Text equality candidates with independent encoders."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest
from compare_null_indexes import INPUT_HASHES

RECIPES = Path(__file__).resolve().parent
LITERALS = [
    "", "a", "A  ", "e", "é", "e\u0301", "ss", "Straße", "カーカ", "😀", "a-b",
    "a" * 255, "\u06d6" * 255, "😀" * 127 + "x", "unmatched",
]


def run(arguments, accepted=True):
    result = subprocess.run(arguments, capture_output=True, text=True, encoding="utf-8", check=False)
    if accepted:
        assert result.returncode == 0 and not result.stderr, (arguments[:2], result.stderr)
        return result.stdout.splitlines()
    assert result.returncode != 0 and not result.stdout, result
    return result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "ranges", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-equality-oracle-") as temporary:
        subprocess.run([
            "javac", "-cp", str(args.jar), "-d", temporary,
            str(RECIPES / "TextEqualityCandidateOracle.java"),
            str(RECIPES / "IndexOracle.java"),
        ], check=True)
        java = [
            "java", "-Dstdout.encoding=UTF-8", "-cp", os.pathsep.join((str(args.jar), temporary)),
            "com.healthmarketscience.jackcess.impl.TextEqualityCandidateOracle",
        ]
        for profile in ["V2000", "V2007", "V2016", "V2019"]:
            extension = "mdb" if profile == "V2000" else "accdb"
            for version in [0, 1]:
                label = f"Text-{profile}-{version}"
                path = args.directory / f"{label}.{extension}"
                original = digest(path)
                assert original == INPUT_HASHES[label]
                descending_a = None
                for index in ["TextAsc", "TextDesc"]:
                    for literal in LITERALS:
                        reference = run([*java, str(path), index, str(version), literal.encode().hex()])
                        expected = [row[2:] for row in reference if row.startswith("C\t")]
                        equal = [row[2:] for row in reference if row.startswith("E\t")]
                        command = [str(args.ranges), str(path), "--index", index,
                                   "--mode", "equality-candidates", "--value", literal]
                        actual = run(command)
                        assert actual == expected, (label, index, repr(literal), "candidate rows/order")
                        if index == "TextDesc" and literal == "a":
                            descending_a = actual
                        assert set(equal) <= set(actual), (label, index, repr(literal), "false negative")
                        projected = run([*command, "--project-id"])
                        assert projected == [row.split("\t", 1)[0] for row in expected]
                        controls += 3
                    for options in [
                        ["--index", "PrimaryKey", "--value", "a"],
                        ["--value", "x" * 256],
                        ["--value", "a", "--start", "a"],
                        [],
                    ]:
                        run([str(args.ranges), str(path), "--mode", "equality-candidates", *options], False)
                        controls += 1
                metadata = run([*java[:-1], "IndexOracle", str(path), "TextIndexed"])
                root = next(int(row.split("\t")[4]) for row in metadata
                            if row.split("\t")[1] == "TextDesc")
                image = bytearray(path.read_bytes())
                page = image[root * 4096:(root + 1) * 4096]
                assert page[0] == 3
                end = next(block * 8 + bit for block, byte in enumerate(page[27:480])
                           for bit in range(8) if byte & (1 << bit))
                child = int.from_bytes(page[480 + end - 4:480 + end], "big")
                assert child != 0 and child != root
                with tempfile.TemporaryDirectory(prefix="tablecraft-equality-corrupt-") as directory:
                    candidate = Path(directory) / path.name
                    image[child * 4096] = 0
                    candidate.write_bytes(image)
                    command = [str(args.ranges), str(candidate), "--index", "TextDesc"]
                    assert run([*command, "--mode", "equality-candidates", "--value", "a"]) == descending_a
                    run([*command, "--mode", "all"], False)
                    image[root * 4096] = 0
                    candidate.write_bytes(image)
                    run([*command, "--mode", "equality-candidates", "--value", "a"], False)
                run([str(args.ranges), str(path), "--index", "TextDesc", "--mode",
                     "equality-candidates", "--value", "a", "--output-bytes", "1"], False)
                controls += 4
                assert digest(path) == original
                controls += 1
                print(label, "passed", flush=True)
    print(json.dumps({"controls": controls, "native_access": "unrun"}))


if __name__ == "__main__":
    main()
