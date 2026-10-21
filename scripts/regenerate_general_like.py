"""Verify or explicitly regenerate the derived General LIKE unit dictionaries.

Uses TableCraft's reviewed weight encoder, not Java, a native Access install or
an external download. Expected artifact hashes in ordinary Rust tests remain
independent review controls and are never rewritten by this script.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess

FAMILIES = {"Legacy": "like-legacy.fst", "General2010": "like-2010.fst"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Replace artifact bytes; default verifies without replacing artifacts")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    completed = subprocess.run(["cargo", "test", "-p", "tablecraft-ace", "--lib",
                               "emit_general_like_dictionaries", "--offline", "--locked",
                               "--", "--ignored", "--nocapture", "--test-threads=1"],
                              cwd=root, capture_output=True, text=True, check=True)
    rows = re.findall(r"GENERAL_LIKE_FST (Legacy|General2010) ([0-9a-f]+)", completed.stdout)
    if [family for family, _ in rows] != list(FAMILIES):
        raise ValueError("incomplete or duplicate generator output")
    generated = {FAMILIES[family]: bytes.fromhex(value) for family, value in rows}
    destination = root / "crates/engines/tablecraft-ace/data/general"
    for name, data in generated.items():
        path = destination / name
        if args.write:
            path.write_bytes(data)
        elif path.read_bytes() != data:
            raise ValueError("derived artifact differs: " + name)
        print(name, len(data), hashlib.sha256(data).hexdigest())
    print("Derived dictionaries written." if args.write else "Derived dictionaries reproduced byte-for-byte; tracked artifacts unchanged.")


if __name__ == "__main__":
    main()
