"""Check Byte/Int16 native range queries against independent stored integer models."""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0

    def check(sql, options, expected=None, diagnostic=None):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, *map(str, options)],
            text=True, encoding="utf-8", capture_output=True,
        )
        if diagnostic:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            assert diagnostic in result.stderr, (sql, diagnostic, result.stderr)
        else:
            assert result.returncode == 0 and not result.stderr, (sql, result.stderr)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[1:] == expected, (sql, rows, expected)
        controls += 1

    for suffix, digest in INPUT_HASHES.items():
        path = args.directory / ("index-" + suffix)
        digest = registered_input_hash(path, digest)
        before = path.read_bytes()
        assert hashlib.sha256(before).hexdigest() == digest
        output = subprocess.check_output(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
             str(RECIPES / "TableDataOracle.java"), str(path), "IndexedRows"],
            text=True, encoding="utf-8",
        )
        stored = [line.split("\t")[1:] for line in output.splitlines()]
        assert len(stored) == 20000
        budgets = ["--database", path, "--native-bytes", "67108864", "--native-rows", "100000",
                   "--native-steps", "10000000", "--execution-steps", "10000000"]
        for column, position, index, kind, prefix, minimum, maximum, point in [
            ("ByteValue", 2, "ByteAsc", "BYTE", "U", 0, 255, 127),
            ("ShortValue", 3, "ShortDesc", "SHORT", "I16:", -32768, 32767, 0),
        ]:
            pairs = []
            for row in stored:
                assert row[0].startswith("I32:")
                tag = row[position]
                assert tag == "N" or tag.startswith(prefix)
                pairs.append((int(row[0][4:]), None if tag == "N" else int(tag[len(prefix):])))
            options = [*budgets, "--index", "IndexedRows", index]
            query = f"SELECT Count(*) AS N,Sum(ID) AS S FROM IndexedRows WHERE [{column}]"
            for value in [minimum, point, maximum]:
                ids = [identifier for identifier, key in pairs if key == value]
                expected = [[str(len(ids)), str(sum(ids)) if ids else "NULL"]]
                check(query + f"={value}", options, expected)
                sql = f"PARAMETERS P {kind}; " + query + "=P"
                check(sql, [*options, "--param", f"P={kind.lower()}:{value}"], expected)
            for lower, upper in [(minimum, maximum), (point, maximum), (point, point), (maximum, minimum)]:
                ids = [identifier for identifier, key in pairs if key is not None and lower <= key < upper]
                check(query + f">={lower} AND [{column}]<{upper}", options,
                      [[str(len(ids)), str(sum(ids)) if ids else "NULL"]])
            for predicate, accepts in [
                ("IS NULL", lambda value: value is None),
                ("IS NOT NULL", lambda value: value is not None),
            ]:
                check(query + " " + predicate, options,
                      [[str(sum(accepts(value) for _, value in pairs)),
                        str(sum(identifier for identifier, value in pairs if accepts(value)))]])
            small = list(options)
            small[small.index("--native-rows") + 1] = "6000"
            count = sum(key == point for _, key in pairs)
            check(f"SELECT Count(*) FROM IndexedRows WHERE [{column}]={point}", small, [[str(count)]])
            check(f"SELECT (SELECT Count(*) FROM IndexedRows WHERE [{column}]={point}),"
                  "(SELECT Count(*) FROM IndexedRows)", small, diagnostic="structural limit")
            check(query + f">={point} AND 1/(ID-ID)=0", options, diagnostic="division by zero")
            invalid = maximum + 1
            check(query + f"<{invalid}", options,
                  [[str(sum(key is not None for _, key in pairs)),
                    str(sum(identifier for identifier, key in pairs if key is not None))]])
        assert path.read_bytes() == before
    print(f"Passed {controls} independent narrow range/parameter/cache/native controls; native Access unrun.")


if __name__ == "__main__":
    main()
