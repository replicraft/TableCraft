"""Compare inert field declarations with original generated ACE-family fixtures."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args):
    return subprocess.check_output(list(map(str, args)), text=True, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ["cli", "jar", "ace2016", "ace2019"]:
        parser.add_argument("--" + key, required=True, type=Path)
    args = parser.parse_args()
    for path, code in [(args.ace2016, 5), (args.ace2019, 6)]:
        original = path.read_bytes()
        assert original[20:24] == bytes([code, 0, 0, 0])
        expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, "docs/testing/AdvancedFieldFixture.java", "--inspect", path)
        actual = run(args.cli, "inspect", path, "--table", "Advanced", "--features")
        records = [row.split("\t") for row in actual.splitlines()[2:]]
        identifiers = []
        for row in records:
            if row[2] != "NONE":
                identifiers.append(int(row[2]))
                row[2] = "PRESENT"
        assert len(identifiers) == len(set(identifiers)) == 3
        assert records == [row.split("\t") for row in expected.splitlines()]
        # A calculated/complex field cannot be silently omitted/coerced to NULL,
        # even in an empty table. No partial result may reach stdout.
        query = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM Advanced", "--database", str(path)], capture_output=True)
        assert query.returncode == 2 and not query.stdout
        complex_expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, "docs/testing/AdvancedFieldFixture.java", "--inspect-complex", path)
        complex_actual = run(args.cli, "inspect", path, "--table", "Advanced", "--complex")
        assert complex_actual.splitlines()[2:] == complex_expected.splitlines()
        assert len(complex_expected.splitlines()) == 3
        print("resolved children", hashlib.sha256(complex_expected.encode()).hexdigest())
        assert path.read_bytes() == original
        print(code, len(records), hashlib.sha256(original).hexdigest(), hashlib.sha256(expected.encode()).hexdigest())
    print("Metadata agrees; inputs unchanged. Complex child values/recalculation and native Access remain unqualified.")


if __name__ == "__main__":
    main()
