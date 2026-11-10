#!/usr/bin/env python3
"""Provision the reviewed official Minisign Linux tool before signing credentials exist."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import platform
import shutil
import tarfile
import urllib.request

import release_contract as contract

VERSION = "0.12"
URL = "https://github.com/jedisct1/minisign/releases/download/0.12/minisign-0.12-linux.tar.gz"
SHA256 = "9a599b48ba6eb7b1e80f12f36b94ceca7c00b7a5173c95c3efc88d9822957e73"
MEMBER = "minisign-linux/x86_64/minisign"
MAX_ARCHIVE_BYTES = 512 * 1024
MAX_EXECUTABLE_BYTES = 1024 * 1024


def install(data, output):
    if len(data) > MAX_ARCHIVE_BYTES or hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Minisign archive digest differs from the reviewed release")
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        members = [member for member in archive.getmembers() if member.name == MEMBER]
        if len(members) != 1 or not members[0].isfile() or not 0 < members[0].size <= MAX_EXECUTABLE_BYTES:
            raise ValueError("Minisign archive has an invalid executable member")
        with archive.extractfile(members[0]) as source:
            binary = source.read(MAX_EXECUTABLE_BYTES + 1)
        if len(binary) != members[0].size:
            raise ValueError("Minisign executable length differs")
        contract.architecture(io.BytesIO(binary), len(binary), "x86_64-unknown-linux-gnu")
    output.mkdir(parents=True, exist_ok=False)
    try:
        destination = output / "minisign"
        with destination.open("xb") as stream:
            stream.write(binary)
        destination.chmod(0o755)
    except BaseException:
        shutil.rmtree(output)
        raise
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if platform.system() != "Linux" or platform.machine() != "x86_64":
            raise ValueError("This signing-tool bootstrap requires Linux x64")
        request = urllib.request.Request(URL, headers={"User-Agent": "TableCraft-packaging"})
        with urllib.request.urlopen(request, timeout=60) as response:
            data = response.read(MAX_ARCHIVE_BYTES + 1)
        binary = install(data, args.output)
        if path_file := os.environ.get("GITHUB_PATH"):
            with open(path_file, "a", encoding="utf-8") as stream:
                stream.write(str(binary.parent.resolve()) + "\n")
        print(f"Provisioned official Minisign {VERSION} with verified SHA-256")
    except (OSError, ValueError, tarfile.TarError) as error:
        parser.exit(1, f"Minisign provisioning failed: {error}\n")


if __name__ == "__main__":
    main()
