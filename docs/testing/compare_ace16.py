"""Generate/compare original ACE 16-family fixtures without native Access claims."""
import argparse
import hashlib
from pathlib import Path
import subprocess


RECIPES = ["Scalar", "Index", "Property", "Relationship", "Validation", "Definition"]
PROFILES = [("V2016", 5), ("V2019", 6)]


def run(*arguments):
    return subprocess.check_output(list(map(str, arguments)), text=True, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--rows", required=True, type=Path)
    parser.add_argument("--ranges", required=True, type=Path)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--generate", action="store_true", help="Require a fresh directory and generate original fixtures first")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, _ in PROFILES:
            for recipe in RECIPES:
                path = args.directory / f"{recipe}-{profile}.accdb"
                run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, f"docs/testing/{recipe}Fixture.java", path, profile)
    paths = {(recipe, profile): args.directory / f"{recipe}-{profile}.accdb" for recipe in RECIPES for profile, _ in PROFILES}
    originals = {}
    for (recipe, profile), path in paths.items():
        data = path.read_bytes()
        expected = dict(PROFILES)[profile]
        if data[4:20] != b"Standard ACE DB\0" or data[20:24] != bytes([expected, 0, 0, 0]):
            raise RuntimeError("Fixture does not have its expected original ACE family header")
        originals[path] = hashlib.sha256(data).hexdigest()
        # Compare all five native catalog fields independently, not just counts.
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, "docs/testing/CatalogOracle.java", path)
        actual = run(args.cli, "inspect", path, "--catalog").splitlines()
        # CLI rows contain common category plus the raw native fields. Locate
        # by the printed header rather than assuming descriptor column positions.
        header = next(line.split("\t") for line in actual if line.startswith("Object ID\t"))
        rows = actual[actual.index("\t".join(header)) + 1:]
        columns = {name: index for index, name in enumerate(header)}
        records = []
        for line in rows:
            cells = line.split("\t")
            selected = [cells[columns[name]] for name in ["Native ID", "Parent ID", "Native type", "Flags", "Name"]]
            selected[3] = str(int(selected[3], 16))
            records.append("\t".join(selected))
        if records != oracle.splitlines():
            raise RuntimeError(f"{profile} {recipe}: native catalog mismatch")
        print(profile, recipe, "catalog", len(records), originals[path], flush=True)
        if recipe == "Scalar":
            actual = run(args.rows, path, "ScalarSamples")
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, "docs/testing/TableDataOracle.java", path, "ScalarSamples")
            if actual != expected:
                raise RuntimeError(f"{profile}: scalar carrier disagreement")
            print(profile, "scalar rows", len(actual.splitlines()), hashlib.sha256(actual.encode()).hexdigest(), flush=True)
    # Reuse existing independent property/relationship/constraint/tree oracles;
    # labels reflect actual file families despite their legacy --ace/--jet slots.
    for recipe, script in [("Property", "properties"), ("Relationship", "relationships"),
            ("Relationship", "record_constraints"), ("Validation", "validation"),
            ("Index", "index_ranges"), ("Definition", "definitions")]:
        executable = args.ranges if recipe == "Index" else args.cli
        flag = "--example" if recipe == "Index" else "--cli"
        jar = [] if script == "record_constraints" else ["--jar", args.jar]
        print(run("python3", f"docs/testing/compare_{script}.py", flag, executable, *jar,
            "--ace", paths[recipe, "V2016"], "--jet", paths[recipe, "V2019"],
            "--ace-label", "ACE16-2016", "--jet-label", "ACE16-2019"), end="", flush=True)
    for profile, _ in PROFILES:
        path = paths["Index", profile]
        expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/IndexRowsOracle.java", path, "Indexed", "Composite", "Category", "ID")
        actual = run(args.cli, "query", "run", "SELECT Category, ID FROM Indexed",
            "--database", path, "--index", "Indexed", "Composite")
        if len(expected.splitlines()) != 1000 or actual != "Category\tID\n" + expected:
            raise RuntimeError("Original two-Int32 compound cursor differs")
        coverage = run(args.cli, "inspect", path, "--table", "Indexed", "--check")
        if "Composite\tChecked\tChecked: exact unique/required\n" not in coverage:
            raise RuntimeError("Original compound index was not checked")
        print(profile, "Composite original ordered rows", 1000,
            hashlib.sha256(expected.encode()).hexdigest(), flush=True)
    for path, before in originals.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != before:
            raise RuntimeError("An original fixture changed during read-only comparison")
    print("All original ACE 16-family fixture hashes unchanged; Microsoft Access remains unrun.")


if __name__ == "__main__":
    main()
