"""Verify or explicitly write the derived French legacy LIKE unit dictionary.

The ignored original Rust generator encodes every raw UTF-16 unit with retained
pattern spaces. It does not invoke Java or Access. Expected hashes in ordinary
Rust tests are separate review controls and are never rewritten here.
"""

import argparse
import hashlib
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Replace the derived artifact")
    parser.add_argument(
        "--captured-output", type=Path,
        help="Read retained generator stdout instead of invoking Cargo",
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.captured_output is None:
        completed = subprocess.run(
            ["cargo", "test", "-p", "tablecraft-ace", "--lib",
             "emit_french_like_dictionary", "--offline", "--locked",
             "--", "--ignored", "--nocapture", "--test-threads=1"],
            cwd=root, capture_output=True, text=True, check=True,
        )
        output = completed.stdout
    else:
        output = args.captured_output.read_text()
    rows = re.findall(r"FRENCH_LIKE_FST ([0-9a-f]+)", output)
    if len(rows) != 1:
        raise ValueError("missing or duplicate French generator output")
    generated = bytes.fromhex(rows[0])
    destination = root / "crates/engines/tablecraft-ace/data/general/like-french-legacy.fst"
    if args.write:
        destination.write_bytes(generated)
    elif destination.read_bytes() != generated:
        raise ValueError("French derived dictionary differs")
    print(destination.name, len(generated), hashlib.sha256(generated).hexdigest())
    print("Derived dictionary written." if args.write else "Derived dictionary matches; artifact unchanged.")


if __name__ == "__main__":
    main()
