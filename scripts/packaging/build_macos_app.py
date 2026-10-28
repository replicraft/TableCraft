#!/usr/bin/env python3
"""Build a Finder open-document receiver around an already-built native CLI."""
import argparse
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile

import release_contract as contract


def build(binary, target, version, output):
    contract.validate_version(version)
    if "apple" not in target or output.exists():
        raise ValueError("A native macOS target and fresh app output are required")
    contract.regular(binary, contract.MAX_EXECUTABLE_BYTES)
    with binary.open("rb") as stream:
        contract.architecture(stream, binary.stat().st_size, target)
    executable = output / "Contents" / "MacOS" / "TableCraft"
    executable.parent.mkdir(parents=True)
    resources = output / "Contents" / "Resources"
    resources.mkdir()
    try:
        with tempfile.TemporaryDirectory(prefix="tablecraft-swift-modules-") as cache:
            subprocess.run([
                "xcrun", "swiftc", "-O", "-warnings-as-errors", "-module-cache-path", cache,
                "-target", ("arm64" if target.startswith("aarch64") else "x86_64") + "-apple-macos13.0",
                str(contract.ROOT / "packaging" / "macos" / "TableCraft.swift"),
                "-o", str(executable),
            ], check=True, timeout=180)
        shutil.copyfile(binary, resources / "tablecraft")
        executable.chmod(0o755)
        (resources / "tablecraft").chmod(0o755)
        (output / "Contents" / "Info.plist").write_bytes(
            plistlib.dumps(contract.macos_info(version), sort_keys=True)
        )
    except BaseException:
        shutil.rmtree(output)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--target", choices=contract.TARGETS, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    build(args.binary, args.target, args.version, args.output)


if __name__ == "__main__":
    main()
