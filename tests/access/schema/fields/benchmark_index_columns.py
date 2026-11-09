"""Time checked isolated native metadata contenders with retained Hyperfine samples.

Run only in a coordinated quiet timing window. Original inputs and all three
executables are hash checked before and after; this command never builds.
"""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--inspection-only", action="store_true")
    parser.add_argument("--reverse-order", action="store_true")
    args = parser.parse_args()
    metadata_path = args.output.with_suffix(".metadata.json")
    assert not args.output.exists() and not metadata_path.exists(), "Preserve previous runs"
    checker = Path(__file__).with_name("verify_index_column_variants.py")
    check = subprocess.check_output([
        "python3", str(checker), "--directory", str(args.directory),
    ], text=True)
    verified = json.loads(check)
    inputs = {
        path.name: digest(path)
        for path in (args.directory / "fixtures").glob("index-columns-*.*")
    }
    variants = ["vec", "arrayvec", "smallvec"]
    if args.reverse_order:
        variants.reverse()
    binaries = {
        name: args.directory / f"index-columns-{name}"
        for name in variants
    }
    original_binaries = {name: digest(path) for name, path in binaries.items()}
    database = args.directory / "fixtures/index-columns-V2016.accdb"
    commands = []
    descriptions = {}
    for width in [1, 2, 10]:
        for count in [1, 24]:
            table = f"Columns{width}Indexes{count}"
            for variant, binary in binaries.items():
                command = [
                    str(binary), "inspect", "--database", str(database),
                    "--table", table, "--iterations", "1000",
                ]
                commands.append((f"inspect-{width}cols-{count}indexes-{variant}", command))
                descriptions[f"{table}-{variant}"] = json.loads(subprocess.check_output([
                    str(binary), "describe", "--database", str(database), "--table", table,
                ], text=True))
        modes = [] if args.inspection_only else variants
        for mode in modes:
            command = [
                str(binaries["vec"]), mode, "--database", str(database),
                "--table", f"Columns{width}Indexes1", "--iterations", "250000",
            ]
            commands.append((f"construct-{width}cols-{mode}", command))

    metadata = {
        "platform": platform.system(),
        "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "runs": 15,
        "warmup": 3,
        "profile": "release, normal Cargo defaults, isolated ACE variants",
        "inspection_iterations": 1000,
        "constructor_iterations": 250000,
        "inspection_only": args.inspection_only,
        "reverse_order": args.reverse_order,
        "native_access": "unrun",
        "allocation_counting": "capacity/spill-derived collection constructors only",
        "timing_scope": "inspection includes ordinary metadata budgets; constructors include capacity/spill bookkeeping",
        "input_sha256": inputs,
        "binary_sha256": original_binaries,
        "correctness": verified,
        "descriptions": descriptions,
        "complete": False,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")
    command = [
        "hyperfine", "--shell=none", "--style=basic", "--warmup", "3",
        "--runs", "15", "--export-json", str(args.output),
    ]
    for name, arguments in commands:
        command += ["--command-name", name, shlex.join(arguments)]
    subprocess.run(command, check=True)
    assert {name: digest(path) for name, path in binaries.items()} == original_binaries
    assert {
        path.name: digest(path)
        for path in (args.directory / "fixtures").glob("index-columns-*.*")
    } == inputs
    metadata["complete"] = True
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")


if __name__ == "__main__":
    main()
