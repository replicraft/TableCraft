"""Compare native GUID point IDs with independent full Jackcess cursor filters."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import uuid


JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
INPUT_SHA256 = ["6805dbcf59eaa8d1ac88c7d5fd710eb131f0382a0d4ce712d7d1c80e1267b079",
                "4300ae1cec647871237c33f454ec7a5ce06f764dc4a874a6251e878bd1f2b900"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "reader", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli", type=Path, help="Also verify query pushdown against independent cursor filters")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    controls = 0
    query_controls = 0
    for path, digest in zip([args.ace2016, args.ace2019], INPUT_SHA256):
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        assert before == digest
        for index in ["GuidAsc", "GuidDesc"]:
            output = subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(Path(__file__).with_name("IndexRowsOracle.java")), str(path), "GuidIndexed", index, "ID", "Value"], text=True)
            rows = []
            for row in output.splitlines():
                identifier, text = row.split("\t")
                rows.append((int(identifier), None if text == "NULL" else uuid.UUID(text.strip("{}"))))
            assert len(rows) == 1000
            keys = {value for _, value in rows if value is not None}
            missing = next(uuid.UUID(int=value) for value in range(1, 10000) if uuid.UUID(int=value) not in keys)
            targets = [None] + sorted(keys, key=lambda value: value.bytes) + [missing]
            for target in ["all", *targets]:
                option = ["--all"] if target == "all" else ["--null"] if target is None else ["--equal", str(target)]
                output = subprocess.check_output([str(args.reader), str(path), "GuidIndexed", index, *option], text=True)
                actual = [int(line) for line in output.splitlines()]
                expected = [identifier for identifier, value in rows if target == "all" or value == target]
                assert actual == expected, (path.name, index, target, actual, expected)
                controls += 1
                if args.cli:
                    if target == "all":
                        predicates = [("", [])]
                    elif target is None:
                        predicates = [(" WHERE [Value] IS NULL", [])]
                    else:
                        literal = "{GUID {" + str(target) + "}}"
                        predicates = [
                            (" WHERE [Value]=" + literal, []),
                            (" WHERE " + literal + "=[Value]", []),
                            (" WHERE [Value]=p", ["--param", "p=guid:" + str(target)]),
                        ]
                    for predicate, parameters in predicates:
                        sql = "SELECT ID FROM GuidIndexed" + predicate
                        if parameters:
                            sql = "PARAMETERS p GUID; " + sql
                        command = [str(args.cli), "query", "run", sql, "--database", str(path),
                                   "--index", "GuidIndexed", index, *parameters]
                        output = subprocess.check_output(command, text=True)
                        lines = output.splitlines()
                        assert lines[0] == "ID", lines[0]
                        actual = [int(line) for line in lines[1:]]
                        assert actual == expected, (path.name, index, predicate, actual, expected)
                        query_controls += 1
        assert hashlib.sha256(path.read_bytes()).hexdigest() == before
    print(f"Passed {query_controls} optional CLI query controls.")
    print(f"Passed {controls} independent GUID point/all/NULL/miss controls on unchanged inputs; native Access unrun.")


if __name__ == "__main__":
    main()
