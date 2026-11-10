#!/usr/bin/env python3
"""Sign exact release archives and verify version-bound Minisign signatures."""
import argparse
import base64
import os
import re
from pathlib import Path
import subprocess
import tempfile

import package_desktop
import release_contract as contract

PUBLIC_KEY = contract.ROOT / "packaging/update-key.pub"
TEST_PUBLIC_KEY = (
    contract.ROOT / "crates/platform/tablecraft-update/src/download/tests/TEST-ONLY.pub"
)
MAX_KEY_BYTES = 4096
MAX_SIGNATURE_BYTES = 4096
MINISIGN_KEY_ID_BYTES = 8
PUBLIC_KEY_MATERIAL_OFFSET = len(b"Ed") + MINISIGN_KEY_ID_BYTES


def names(version, product):
    contract.validate_version(version)
    archive_name = (package_desktop.archive_name if product == "desktop"
                    else contract.archive_name)
    return sorted(archive_name(version, target) for target in contract.TARGETS)


def comment(version, name):
    return f"tablecraft-update {version} {name}"


def key_bytes(text):
    lines = text.splitlines()
    if len(lines) != 2 or not lines[0].startswith("untrusted comment:"):
        raise ValueError("Expected a Minisign public key file")
    raw = base64.b64decode(lines[1], validate=True)
    if len(raw) != 42 or raw[:2] != b"Ed":
        raise ValueError("Expected an Ed25519 Minisign public key")
    return raw


def check_key(path, *, production):
    text = contract.regular(path, MAX_KEY_BYTES).read_text("utf-8")
    raw = key_bytes(text)
    if production:
        fixture = contract.regular(TEST_PUBLIC_KEY, MAX_KEY_BYTES).read_text("utf-8")
        # Compare the Ed25519 key itself: comments and the eight-byte Minisign
        # key ID can change without changing which private key is trusted.
        if raw[PUBLIC_KEY_MATERIAL_OFFSET:] == key_bytes(fixture)[PUBLIC_KEY_MATERIAL_OFFSET:]:
            raise ValueError("Production signing refuses the known test-only public key")
        lines = text.splitlines()
        label = re.sub(r"[^A-Z0-9]", "", lines[0].upper())
        if "TESTONLY" in label:
            raise ValueError("Production signing refuses a test-only public key")
        trusted = contract.regular(PUBLIC_KEY, MAX_KEY_BYTES).read_text("utf-8")
        if text != trusted:
            raise ValueError("Production signing requires the pinned TableCraft release key")
    return raw


def execute(command):
    # Captured diagnostics may contain key details; report only the operation failure.
    environment = os.environ.copy()
    environment.pop("TABLECRAFT_UPDATE_SIGNING_KEY", None)
    result = subprocess.run(
        command, stdin=subprocess.DEVNULL, env=environment,
        capture_output=True, check=False, timeout=120,
    )
    if result.returncode:
        raise ValueError("Minisign operation failed")


def verify_one(tool, public_key, archive, signature, version):
    lines = contract.regular(signature, MAX_SIGNATURE_BYTES).read_text("utf-8").splitlines()
    if len(lines) != 4 or lines[2] != f"trusted comment: {comment(version, archive.name)}":
        raise ValueError("Signature is not bound to the requested version and archive")
    payload = base64.b64decode(lines[1], validate=True)
    if len(payload) != 74 or payload[:2] != b"ED":
        raise ValueError("Only prehashed Ed25519 Minisign signatures are supported")
    execute([tool, "-V", "-q", "-p", str(public_key), "-m", str(archive), "-x", str(signature)])


def run(version, product, assets, output, public_key, tool, *, signing, dry_run):
    check_key(public_key, production=not dry_run)
    archives = [contract.regular(assets / name) for name in names(version, product)]
    if signing:
        # Refuse existing directories so a failed run cannot leave stale trusted signatures.
        output.mkdir(parents=True, exist_ok=False)
        created = []
        try:
            secret = os.environ.get("TABLECRAFT_UPDATE_SIGNING_KEY", "")
            if not secret.strip():
                raise ValueError("TABLECRAFT_UPDATE_SIGNING_KEY is required")
            if len(secret.encode("utf-8")) > MAX_KEY_BYTES:
                raise ValueError("Signing key exceeds the key-size limit")
            # An isolated signing environment provides an unencrypted Minisign key.
            # File permissions protect it; never put the secret in argv or logs.
            with tempfile.TemporaryDirectory(prefix="tablecraft-sign-") as temporary:
                key_file = Path(temporary) / "key"
                with open(
                    key_file, "x", encoding="utf-8",
                    opener=lambda path, flags: os.open(path, flags, 0o600),
                ) as stream:
                    stream.write(secret)
                for archive in archives:
                    signature = output / (archive.name + ".minisig")
                    created.append(signature)
                    execute([tool, "-S", "-s", str(key_file), "-m", str(archive),
                             "-x", str(signature), "-t", comment(version, archive.name)])
                    verify_one(tool, public_key, archive, signature, version)
        except BaseException:
            for path in created:
                path.unlink(missing_ok=True)
            output.rmdir()
            raise
    else:
        expected = {archive.name + ".minisig" for archive in archives}
        if output.is_symlink() or not output.is_dir() or {p.name for p in output.iterdir()} != expected:
            raise ValueError("Signature directory does not match the complete archive set")
        for archive in archives:
            verify_one(tool, public_key, archive, output / (archive.name + ".minisig"), version)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("sign", "verify"))
    parser.add_argument("--version", required=True)
    parser.add_argument("--product", choices=("cli", "desktop"), required=True)
    parser.add_argument("--assets", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--public-key", type=Path, default=PUBLIC_KEY)
    parser.add_argument("--minisign", default="minisign")
    parser.add_argument("--dry-run", action="store_true", help="Use an explicit disposable test key; never publish")
    args = parser.parse_args()
    try:
        run(args.version, args.product, args.assets, args.output, args.public_key,
            args.minisign, signing=args.command == "sign", dry_run=args.dry_run)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Update signing failed: {error}\n")


if __name__ == "__main__":
    main()
