"""Compare CLI table browsing against independent Jackcess scalar snapshots.

Uses the reviewed Jackcess artifact and original ScalarFixture recipe. Generated
files remain local. This establishes independent reading, not native Access SQL
or shared-file qualification. Scalar float formatting covers this recipe's exact
binary fractions only; it is not a general cross-language float formatter.
"""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import struct
import subprocess
import uuid

PROFILES = [("V2000", "mdb"), ("V2016", "accdb"), ("V2019", "accdb")]
RECIPES = Path(__file__).resolve().parent


def run(*args):
    return command_output(args, subprocess.check_output(list(map(str, args)), encoding="utf-8"))


def escaped(text):
    simple = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
    return "".join(simple.get(ch, f"\\u{{{ord(ch):04x}}}" if ord(ch) < 32 or 127 <= ord(ch) <= 159 else ch) for ch in text)


def fixed(coefficient, scale):
    negative = coefficient.startswith("-")
    digits = coefficient.lstrip("-").zfill(scale + 1)
    result = digits if scale == 0 else digits[:-scale] + "." + digits[-scale:]
    return ("-" if negative else "") + result


def display(tag):
    if tag == "N":
        return "NULL"
    if tag.startswith("B"):
        return {"B0": "FALSE", "B1": "TRUE"}[tag]
    if tag.startswith("I"):
        return tag.split(":", 1)[1]
    if tag.startswith("U"):
        return tag[1:]
    if tag.startswith("C"):
        return fixed(tag[1:], 4)
    if tag.startswith("Q"):
        coefficient, scale = tag[1:].split(":")
        return fixed(coefficient, int(scale))
    if tag.startswith(("F", "D")):
        single = tag[0] == "F"
        value = struct.unpack(">f" if single else ">d", bytes.fromhex(tag[1:]))[0]
        return format(value, ".9g" if single else ".17g")
    if tag.startswith("T"):
        return '"' + escaped(bytes.fromhex(tag[1:]).decode("utf-8")) + '"'
    if tag.startswith("X"):
        return "0x" + tag[1:]
    if tag.startswith("G"):
        return "{" + str(uuid.UUID(tag[1:])) + "}"
    if tag.startswith(("Y", "Z")):
        return "#" + tag[1:] + "#"
    raise ValueError("Unsupported oracle tag")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--profile", choices=["V2000", "V2007", "V2016", "V2019"])
    args = parser.parse_args()
    profiles = [(args.profile, "mdb" if args.profile == "V2000" else "accdb")] if args.profile else PROFILES
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in profiles:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "ScalarFixture.java", args.directory / f"Scalar-{profile}.{suffix}", profile)
    for profile, suffix in profiles:
        path = args.directory / f"Scalar-{profile}.{suffix}"
        before = path.read_bytes()
        family = {"V2000": 1, "V2007": 2, "V2016": 5, "V2019": 6}[profile]
        assert before[20:24] == bytes([family, 0, 0, 0]), "Wrong original family"
        def oracle(recipe, *extra):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / f"{recipe}.java", path, *extra)
        catalog = [line.split("\t") for line in oracle("CatalogOracle").splitlines()]
        names = sorted(escaped(row[4]) + "\t" + ("System" if int(row[3]) & 0x80000002 else "User") for row in catalog if int(row[2]) in (1, 4, 6))
        actual = run(args.cli, "tables", path, "--show-system").splitlines()
        assert actual[0] == "Name\tKind" and sorted(actual[1:]) == names, "Table inventory mismatch"
        columns = [escaped(line.split("\t")[1]) for line in oracle("TableSchemaOracle", "ScalarSamples").splitlines()]
        header = "\t".join(columns) + "\n"
        rows = ["\t".join(display(tag) for tag in line.split("\t")[1:]) + "\n" for line in oracle("TableDataOracle", "ScalarSamples").splitlines()]
        cases = [([], rows), (["--limit", "1"], rows[:1]), (["--offset", "1", "--limit", "1"], rows[1:2]), (["--offset", str(len(rows))], []), (["--offset", "4294967295"], [])]
        for options, selected in cases:
            assert run(args.cli, "rows", path, "ScalarSamples", *options) == header + "".join(selected), "Scalar page mismatch"
        # Reuse the native query path as an additional integration regression;
        # expected bytes still come from the independent Java row oracle above.
        assert run(args.cli, "query", "run", "SELECT * FROM ScalarSamples ORDER BY ID", "--database", path) == header + "".join(rows), "SELECT mismatch"
        assert path.read_bytes() == before, "Read command changed original bytes"
        print(f"{profile}: inventory + 5 pages + SELECT agree; unchanged SHA-256 {hashlib.sha256(before).hexdigest()}")


if __name__ == "__main__":
    main()
