"""Compare fresh-copy fixed Int32 edits with Jackcess; never modify the input."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ["example", "cli", "jar", "jet"]:
        parser.add_argument("--" + argument, required=True, type=Path)
    args = parser.parse_args()
    before = args.jet.read_bytes()

    def inspect(path):
        return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            "docs/testing/UpdateFixture.java", "--inspect", str(path)], text=True)

    original = "1\t10\t8400\t12288\n2\t20\t8400\t12288\n3\t30\t8400\t12288\n"
    if inspect(args.jet) != original:
        raise RuntimeError("Unexpected original recipe contents")
    with tempfile.TemporaryDirectory(prefix="tablecraft-native-updates-") as directory:
        output = Path(directory) / "updated.mdb"
        subprocess.run([str(args.example), str(args.jet), str(output), "Items", "Score", "1", "20", "27"], check=True)
        if inspect(output) != original.replace("2\t20\t", "2\t27\t"):
            raise RuntimeError("Independent reopen disagrees")
        after = output.read_bytes()
        if len(after) != len(before) or [(a, b) for a, b in zip(before, after) if a != b] != [(20, 27)]:
            raise RuntimeError("Unexpected byte changes outside the one-byte replacement")
        subprocess.run([str(args.cli), "inspect", str(output), "--table", "Items", "--check"], check=True, stdout=subprocess.DEVNULL)
        for number, (column, expected, value) in enumerate([("Score", 20, 101), ("Score", 99, 27), ("Id", 2, 27)]):
            rejected = Path(directory) / f"rejected-{number}.mdb"
            result = subprocess.run([str(args.example), str(args.jet), str(rejected), "Items", column, "1", str(expected), str(value)], capture_output=True)
            if result.returncode == 0 or rejected.exists():
                raise RuntimeError("Unqualified edit published an output")
        print("Jet4 independent row/index reopen; exact byte preservation; rule/stale/index rejections passed")
        print("before", hashlib.sha256(before).hexdigest(), "after", hashlib.sha256(after).hexdigest())
    if args.jet.read_bytes() != before:
        raise RuntimeError("Original fixture changed")


if __name__ == "__main__":
    main()
