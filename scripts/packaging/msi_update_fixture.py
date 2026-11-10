#!/usr/bin/env python3
"""Build a clearly named, next-version MSI fixture outside the release workspace.

The executable remains the current workspace version. Only disposable MSI and
manifest metadata advance; this qualifies installer workflow, not binary-version
compatibility. Production version and asset naming checks remain unchanged.
"""
import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import package_windows as windows
import release_contract as contract

FIXTURE_NAME = "TEST-ONLY-tablecraft-msi-update.msi"
RECEIPT_NAME = "TEST-ONLY.json"


def next_patch(current):
    windows.msi_version(current)
    major, minor, patch = (int(part) for part in current.split("."))
    if patch >= 65535:
        raise ValueError("The disposable MSI fixture needs a representable next patch version")
    return f"{major}.{minor}.{patch + 1}"


@contextmanager
def fixture_workspace(parent, version, original_version):
    if not contract.VERSION.fullmatch(version):
        raise ValueError("Fixture workspace version must be safe SemVer")
    original_root = contract.ROOT
    root = parent / "fixture-workspace"
    root.mkdir()
    (root / "Cargo.toml").write_text(
        f'[workspace.package]\nversion = "{version}"\n', encoding="utf-8"
    )
    shutil.copyfile(original_root / "LICENSE", root / "LICENSE")
    notes = root / "docs/releases"
    notes.mkdir(parents=True)
    (notes / f"v{version}.md").write_text(
        f"TEST ONLY: current executable {original_version}; disposable MSI metadata {version}.\n"
        "This fixture is never a distributable release.\n", encoding="utf-8"
    )
    try:
        # Only this isolated engineering process selects synthetic metadata.
        # Production package_windows keeps its unchanged repository root/guards.
        contract.ROOT = root
        windows.msi_version(version)
        yield root
    finally:
        contract.ROOT = original_root


def build(version, target, binary, notices, icon, output, tool="wix"):
    original_root = contract.ROOT
    current = contract.workspace_version()
    if version != next_patch(current):
        raise ValueError("Fixture metadata must be exactly the next stable MSI patch")
    if output.exists() or output.is_symlink():
        raise FileExistsError("The disposable fixture output must be new")
    if output.resolve().is_relative_to(original_root.resolve()):
        raise ValueError("Disposable MSI fixtures must stay outside the release workspace")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".tablecraft-msi-fixture-", dir=output.parent) as temporary:
        temporary = Path(temporary)
        with fixture_workspace(temporary, version, current):
            built = windows.package(version, target, binary, notices, icon, temporary / "built", tool)
        receipt = {
            "schema": 1,
            "fixture_only": True,
            "binary_workspace_version": current,
            "declared_fixture_version": version,
            "target": target,
            "binary_sha256": contract.digest(binary),
            "msi_sha256": contract.digest(built),
            "release_asset": False,
        }
        created = []
        output.mkdir(exist_ok=False)
        try:
            destination = output / FIXTURE_NAME
            os.link(built, destination)
            created.append(destination)
            with (output / RECEIPT_NAME).open("x", encoding="utf-8", newline="\n") as stream:
                created.append(output / RECEIPT_NAME)
                stream.write(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        except BaseException:
            for path in reversed(created):
                path.unlink(missing_ok=True)
            try:
                output.rmdir()
            except OSError:
                pass
            raise
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=windows.ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--icon", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--wix", default="wix")
    args = parser.parse_args()
    try:
        print(build(args.version, args.target, args.binary, args.notices, args.icon, args.output, args.wix))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Disposable native MSI fixture failed: {error}\n")


if __name__ == "__main__":
    main()
