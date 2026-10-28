#!/usr/bin/env python3
"""Package, smoke-test and assemble the complete six-platform CLI release."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import shutil
import stat
import subprocess
import tarfile
import tempfile
import zipfile

import release_contract as contract

MANIFEST = "RELEASE.json"
SUMS = "SHA256SUMS.txt"
LICENSE_BUNDLE = "ThirdPartyLicenses.txt"
MAX_MANIFEST_BYTES = 1024 * 1024
COPY_BUFFER_BYTES = 1024 * 1024
SMOKE_TIMEOUT_SECONDS = 30
EXECUTABLE_MODE = 0o755
DATA_MODE = 0o644
ZIP_ENCRYPTED_FLAG = 0x01
ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)


def smoke(binary, version):
    for argument in ("--version", "--help"):
        result = subprocess.run(
            [str(binary.resolve()), argument], capture_output=True, text=True,
            encoding="utf-8", timeout=SMOKE_TIMEOUT_SECONDS,
        )
        if result.returncode != 0 or result.stderr:
            raise ValueError(f"Extracted CLI {argument} failed: {result.stderr}")
        if argument == "--version" and result.stdout.strip() != f"tablecraft {version}":
            raise ValueError("Extracted CLI version does not match the archive")
        if argument == "--help" and ("Usage:" not in result.stdout or "query" not in result.stdout):
            raise ValueError("Extracted CLI help is incomplete")


def checksum(path):
    return f"{contract.digest(path)}  {path.name}\n".encode("ascii")


def package(version, target, binary, output, macos_app=None):
    contract.validate_version(version)
    contract.regular(binary, contract.MAX_EXECUTABLE_BYTES)
    with binary.open("rb") as stream:
        contract.architecture(stream, binary.stat().st_size, target)
    files = contract.legal_payload()
    executable = contract.executable_name(target)
    files[executable] = binary
    files.update(contract.static_handler_payload(target))
    if "apple" in target:
        if macos_app is None:
            raise ValueError("macOS releases require the native Finder receiver app")
        for name in contract.handler_names(target):
            relative = Path(name).relative_to("TableCraft.app")
            files[name] = contract.regular(macos_app / relative)
    metadata = contract.manifest(version, target, files)
    output.mkdir(parents=True, exist_ok=True)
    standalone = output / LICENSE_BUNDLE
    license_source = files[LICENSE_BUNDLE]
    if standalone.exists() or standalone.is_symlink():
        contract.regular(standalone, contract.MAX_LEGAL_BYTES)
        if contract.digest(standalone) != contract.digest(license_source):
            raise ValueError("Existing standalone third-party licenses differ")
    archive = output / contract.archive_name(version, target)
    if archive.exists() or archive.is_symlink() or archive.with_name(archive.name + ".sha256").exists():
        raise ValueError("Release assets must be written to fresh paths")
    try:
        if "windows" in target:
            with zipfile.ZipFile(archive, "x", zipfile.ZIP_DEFLATED) as stream:
                for name, path in sorted(files.items()):
                    info = zipfile.ZipInfo(name, date_time=ZIP_EPOCH)
                    info.create_system = 3
                    mode = EXECUTABLE_MODE if contract.executable_member(name, target) else DATA_MODE
                    info.external_attr = (stat.S_IFREG | mode) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    with path.open("rb") as source, stream.open(info, "w", force_zip64=True) as destination:
                        shutil.copyfileobj(source, destination)
                stream.writestr(MANIFEST, metadata)
        else:
            with archive.open("xb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as zipped:
                with tarfile.open(fileobj=zipped, mode="w") as stream:
                    for name, path in sorted(files.items()):
                        info = tarfile.TarInfo(name)
                        info.size = path.stat().st_size
                        info.mode = EXECUTABLE_MODE if contract.executable_member(name, target) else DATA_MODE
                        with path.open("rb") as source:
                            stream.addfile(info, source)
                    info = tarfile.TarInfo(MANIFEST)
                    info.size = len(metadata)
                    info.mode = DATA_MODE
                    stream.addfile(info, io.BytesIO(metadata))
        verify_archive(archive, version, target)
        archive.with_name(archive.name + ".sha256").write_bytes(checksum(archive))
        if not standalone.exists():
            with license_source.open("rb") as source, standalone.open("xb") as destination:
                shutil.copyfileobj(source, destination)
    except BaseException:
        archive.unlink(missing_ok=True)
        raise
    return archive


def copy_member(source, destination, maximum):
    total = 0
    hasher = hashlib.sha256()
    with destination.open("xb") as output:
        while block := source.read(COPY_BUFFER_BYTES):
            total += len(block)
            if total > maximum:
                raise ValueError("Archive member exceeds its release limit")
            output.write(block)
            hasher.update(block)
    return hasher.hexdigest()


def verify_archive(archive, version, target, run_smoke=False):
    contract.regular(archive)
    if archive.name != contract.archive_name(version, target):
        raise ValueError("Unexpected archive name")
    executable = contract.executable_name(target)
    legal = contract.legal_payload()
    expected = {*legal, executable, MANIFEST, *contract.handler_names(target)}
    with tempfile.TemporaryDirectory(prefix="tablecraft-release-verify-") as temporary:
        destination = Path(temporary)
        seen = set()
        hashes = {}

        def extract(name, source, size, mode):
            if name not in expected or name in seen:
                raise ValueError(f"Unexpected or duplicate archive member: {name}")
            seen.add(name)
            maximum = (contract.MAX_EXECUTABLE_BYTES if contract.executable_member(name, target)
                       else MAX_MANIFEST_BYTES if name == MANIFEST else contract.MAX_LEGAL_BYTES)
            if size <= 0 or size > maximum:
                raise ValueError("Archive member has an invalid size")
            path = destination / name
            path.parent.mkdir(parents=True, exist_ok=True)
            hashes[name] = copy_member(source, path, maximum)
            if path.stat().st_size != size:
                raise ValueError("Archive member length mismatch")
            if contract.executable_member(name, target):
                if mode != EXECUTABLE_MODE:
                    raise ValueError("Executable archive permissions must be 0755")
                path.chmod(EXECUTABLE_MODE)

        if "windows" in target:
            with zipfile.ZipFile(archive) as stream:
                for entry in stream.infolist():
                    mode = entry.external_attr >> 16
                    if entry.is_dir() or stat.S_IFMT(mode) not in (0, stat.S_IFREG) or entry.flag_bits & ZIP_ENCRYPTED_FLAG:
                        raise ValueError("Directories, special files, links and encrypted ZIP entries are refused")
                    with stream.open(entry) as source:
                        extract(entry.filename, source, entry.file_size, stat.S_IMODE(mode))
        else:
            with tarfile.open(archive, "r:gz") as stream:
                for entry in stream:
                    if not entry.isfile():
                        raise ValueError("Only regular archive files are admitted")
                    with stream.extractfile(entry) as source:
                        extract(entry.name, source, entry.size, entry.mode)
        if seen != expected:
            raise ValueError("Archive is missing required binary/license/manifest resources")
        metadata = json.loads((destination / MANIFEST).read_text())
        fixed = {**legal, **contract.static_handler_payload(target)}
        expected_hashes = {name: contract.digest(path) for name, path in fixed.items()}
        for name, value in expected_hashes.items():
            if hashes[name] != value:
                raise ValueError(f"Changed release resource: {name}")
        hashes.pop(MANIFEST)
        expected_metadata = {
            "product": "tablecraft-cli", "version": version, "target": target, "files": hashes,
        }
        if metadata != expected_metadata:
            raise ValueError("Archive manifest/version/target/hash mismatch")
        binary = destination / executable
        with binary.open("rb") as source:
            contract.architecture(source, binary.stat().st_size, target)
        if "apple" in target:
            launcher = destination / contract.MACOS_LAUNCHER
            with launcher.open("rb") as source:
                contract.architecture(source, launcher.stat().st_size, target)
            if hashes[contract.MACOS_CLI] != hashes[executable]:
                raise ValueError("Bundled Finder CLI differs from the portable executable")
            if plistlib.loads((destination / contract.MACOS_INFO).read_bytes()) != contract.macos_info(version):
                raise ValueError("Finder association metadata differs from the release contract")
        if run_smoke:
            smoke(binary, version)


def finalize(version, output, verify_only=False):
    contract.validate_version(version)
    names = {contract.archive_name(version, target) for target in contract.TARGETS}
    required = names | {name + ".sha256" for name in names} | {LICENSE_BUNDLE}
    actual = {path.name for path in output.iterdir()}
    allowed = required | ({SUMS} if verify_only else set())
    if actual != allowed:
        raise ValueError(f"Release set differs: missing={allowed - actual}, extra={actual - allowed}")
    bundle = contract.regular(output / LICENSE_BUNDLE, contract.MAX_LEGAL_BYTES)
    if contract.digest(bundle) != contract.digest(contract.legal_payload()[LICENSE_BUNDLE]):
        raise ValueError("Standalone third-party licenses differ from the release payload")
    for target in contract.TARGETS:
        archive = output / contract.archive_name(version, target)
        verify_archive(archive, version, target)
        sidecar = contract.regular(output / (archive.name + ".sha256"))
        if sidecar.read_bytes() != checksum(archive):
            raise ValueError(f"Checksum sidecar mismatch: {archive.name}")
    checksums = b"".join(checksum(contract.regular(output / name)) for name in sorted(required))
    if verify_only:
        if contract.regular(output / SUMS).read_bytes() != checksums:
            raise ValueError("Complete checksum manifest mismatch")
    else:
        with (output / SUMS).open("xb") as stream:
            stream.write(checksums)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    validate = commands.add_parser("validate")
    validate.add_argument("--tag")
    for command in ("package", "verify-archive"):
        child = commands.add_parser(command)
        child.add_argument("--version", required=True)
        child.add_argument("--target", choices=contract.TARGETS, required=True)
        if command == "package":
            child.add_argument("--binary", type=Path, required=True)
            child.add_argument("--output", type=Path, required=True)
            child.add_argument("--macos-app", type=Path)
        else:
            child.add_argument("--archive", type=Path, required=True)
            child.add_argument("--smoke", action="store_true")
    for command in ("finalize", "verify"):
        child = commands.add_parser(command)
        child.add_argument("--version", required=True)
        child.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "validate":
            version = contract.workspace_version()
            contract.validate_version(version, args.tag)
            contract.legal_payload()
            if output := os.environ.get("GITHUB_OUTPUT"):
                with Path(output).open("a", encoding="utf-8", newline="\n") as stream:
                    stream.write(f"version={version}\n")
            print(version)
        elif args.command == "package":
            print(package(args.version, args.target, args.binary, args.output, args.macos_app))
        elif args.command == "verify-archive":
            contract.validate_version(args.version)
            verify_archive(args.archive, args.version, args.target, args.smoke)
        else:
            finalize(args.version, args.output, verify_only=args.command == "verify")
    except (OSError, ValueError, subprocess.SubprocessError, zipfile.BadZipFile, tarfile.TarError) as error:
        parser.exit(1, f"Release packaging failed: {error}\n")


if __name__ == "__main__":
    main()
