"""Check the registered saved-read inventory against inert parameter getters.

Only TableCraft parses SQL. Neither reader executes saved queries or VBA.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess

from compare_example_saved_join import DATABASE_SHA256, JAR_SHA256, RECIPES


def run(*command):
    return subprocess.run(
        list(map(str, command)), capture_output=True, text=True,
        encoding="utf-8", timeout=30, check=False,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument(
        "--database", type=Path,
        default=RECIPES.parent / "examples/Access_Example_VBA.accdb",
    )
    args = parser.parse_args()
    before = args.database.read_bytes()
    if hashlib.sha256(before).hexdigest() != DATABASE_SHA256:
        raise RuntimeError("Registered database hash mismatch")
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != JAR_SHA256:
        raise RuntimeError("Reviewed JAR hash mismatch")

    report = run(
        "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        RECIPES / "SavedParameterOracle.java", args.database,
    )
    if report.returncode:
        raise RuntimeError("Independent parameter getter failed: " + report.stderr)

    definitions = {}
    parameters = None
    for line in report.stdout.splitlines():
        fields = line.split("\t")
        if len(fields) == 3 and fields[0] == "QUERY":
            name = bytes.fromhex(fields[1]).decode("utf-8")
            if name in definitions:
                raise RuntimeError("Duplicate independent query name")
            parameters = []
            definitions[name] = (fields[2], parameters)
        elif len(fields) == 2 and fields[0] == "PARAMETER" and parameters is not None:
            parameters.append(bytes.fromhex(fields[1]).decode("utf-8"))
        else:
            raise RuntimeError("Invalid independent parameter report")
    if len(definitions) != 59:
        raise RuntimeError("Registered query inventory changed")

    reads = rejected = value_parameters = 0
    for name, (kind, parameters) in definitions.items():
        result = run(args.cli, "query", "saved", "--database", args.database, name)
        if kind not in {"SELECT", "UNION", "CROSS_TAB"}:
            if result.returncode != 2 or result.stdout:
                raise RuntimeError("Non-read definition became a saved read: " + name)
            rejected += 1
            continue
        if result.returncode:
            raise RuntimeError("Saved read reconstruction failed: " + name + ": " + result.stderr)

        parsed = run(args.cli, "query", "parse", result.stdout)
        if parsed.returncode or parsed.stdout != result.stdout:
            raise RuntimeError("Owned saved read did not round-trip: " + name)
        if parameters:
            declaration = "PARAMETERS " + ", ".join(parameters) + "; SELECT 0;"
            expected = run(args.cli, "query", "parse", declaration)
            suffix = " SELECT 0;\n"
            if expected.returncode or not expected.stdout.endswith(suffix):
                raise RuntimeError("Independent parameter declaration did not parse: " + name)
            prefix = expected.stdout.removesuffix(suffix)
            if not result.stdout.startswith(prefix + " "):
                raise RuntimeError("Saved parameter declarations differ: " + name)
            value_parameters += sum(parameter.endswith(" Value") for parameter in parameters)
        elif result.stdout.startswith("PARAMETERS "):
            raise RuntimeError("Saved reconstruction invented parameters: " + name)
        reads += 1

    if reads != 51 or rejected != 8 or not value_parameters:
        raise RuntimeError("Registered read-family or VALUE coverage changed")
    if args.database.read_bytes() != before:
        raise RuntimeError("Registered database changed")
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != JAR_SHA256:
        raise RuntimeError("Reviewed JAR changed")
    print(f"Passed {reads} saved-read round trips and parameter comparisons; "
          f"{rejected} non-read refusals; {value_parameters} VALUE declarations.")
    print("Originals unchanged. SQL execution, VBA and native Access qualification unrun.")


if __name__ == "__main__":
    main()
