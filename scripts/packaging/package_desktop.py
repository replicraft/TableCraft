#!/usr/bin/env python3
"""Portable desktop archives; no installation, association registration or GUI launch.

Notices are generated from locked Cargo sources and maintained reviewed policy
unless a reviewed bundle is supplied explicitly. macOS requires a real ICNS.
These checks establish archive integrity, not license completeness,
interactive compatibility, signing or native file-open event support.
"""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import plistlib
import shutil
import stat
import struct
import subprocess
import tarfile
import tempfile
import zipfile
import zlib

import release_contract as contract
import desktop_notices
from file_associations import APPLICATION_ID

MANIFEST = "RELEASE.json"
NOTICES = "ThirdPartyLicenses.txt"
PRODUCT = "tablecraft-desktop"
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_ICON_BYTES = 16 * 1024 * 1024
MAX_ARCHIVE_BYTES = (
    contract.MAX_EXECUTABLE_BYTES
    + 2 * contract.MAX_LEGAL_BYTES
    + MAX_ICON_BYTES
    + 2 * MAX_MANIFEST_BYTES
    + 1024 * 1024
)
COPY_BYTES = 1024 * 1024
EXECUTABLE_MODE = 0o755
DATA_MODE = 0o644
ZIP_EPOCH = (1980, 1, 1, 0, 0, 0)
ICNS_SIZES = {
    b"icp4": 16,
    b"icp5": 32,
    b"icp6": 64,
    b"ic07": 128,
    b"ic08": 256,
    b"ic09": 512,
    b"ic10": 1024,
    b"ic11": 32,
    b"ic12": 64,
    b"ic13": 256,
    b"ic14": 512,
}
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def executable_path(target):
    if target not in contract.TARGETS:
        raise ValueError("Unsupported desktop target")
    if "apple" in target:
        return "TableCraft.app/Contents/MacOS/tablecraft-desktop"
    return "tablecraft-desktop.exe" if "windows" in target else "tablecraft-desktop"


def resource_path(target, name):
    if "apple" in target:
        return "TableCraft.app/Contents/Resources/" + name
    return name


def archive_name(version, target):
    suffix = ".zip" if "windows" in target or "apple" in target else ".tar.gz"
    return f"{PRODUCT}-{version}-{contract.TARGETS[target]}{suffix}"


def bundle_metadata(version):
    numeric = version.split("-", 1)[0]
    return {
        "CFBundleIdentifier": APPLICATION_ID,
        "CFBundleName": "TableCraft",
        "CFBundleDisplayName": "TableCraft",
        "CFBundleExecutable": "tablecraft-desktop",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": numeric,
        "CFBundleVersion": numeric,
        "CFBundleIconFile": "TableCraft.icns",
        "NSHighResolutionCapable": True,
    }


def validate_png(data, dimension):
    """Require real noninterlaced RGB/RGBA PNG pixels, not a signature placeholder."""
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("ICNS chunks must contain PNG images")
    offset = len(PNG_SIGNATURE)
    chunks = []
    compressed = bytearray()
    while offset < len(data):
        if len(data) - offset < 12:
            raise ValueError("Truncated icon PNG chunk")
        size = struct.unpack_from(">I", data, offset)[0]
        end = offset + 12 + size
        if end > len(data):
            raise ValueError("Truncated icon PNG payload")
        tag = data[offset + 4:offset + 8]
        if not all(65 <= value <= 90 or 97 <= value <= 122 for value in tag):
            raise ValueError("Invalid icon PNG chunk type")
        body = data[offset + 8:end - 4]
        crc = struct.unpack_from(">I", data, end - 4)[0]
        if zlib.crc32(tag + body) != crc:
            raise ValueError("Invalid icon PNG checksum")
        chunks.append(tag)
        if tag == b"IEND" and size != 0:
            raise ValueError("Invalid icon PNG terminator")
        if tag not in (b"IHDR", b"IDAT", b"IEND") and not tag[0] & 0x20:
            raise ValueError("Unsupported critical icon PNG chunk")
        if tag == b"IHDR":
            if size != 13:
                raise ValueError("Invalid icon PNG header")
            width, height, depth, color, compression, filtering, interlace = struct.unpack(
                ">IIBBBBB", body,
            )
            if (
                (width, height) != (dimension, dimension)
                or depth != 8
                or color not in (2, 6)
                or any((compression, filtering, interlace))
            ):
                raise ValueError("Unsupported icon PNG pixel format or dimensions")
            channels = 3 if color == 2 else 4
        elif tag == b"IDAT":
            compressed.extend(body)
        offset = end
    if (
        not chunks
        or chunks[0] != b"IHDR"
        or chunks[-1] != b"IEND"
        or chunks.count(b"IHDR") != 1
        or chunks.count(b"IEND") != 1
        or not compressed
    ):
        raise ValueError("Incomplete icon PNG")
    maximum = dimension * (dimension * channels + 1)
    decoder = zlib.decompressobj()
    pixels = decoder.decompress(compressed, maximum + 1)
    if (
        len(pixels) != maximum
        or not decoder.eof
        or decoder.unused_data
        or decoder.unconsumed_tail
    ):
        raise ValueError("Invalid or oversized icon PNG pixel data")
    stride = dimension * channels + 1
    if any(pixels[index] > 4 for index in range(0, maximum, stride)):
        raise ValueError("Invalid icon PNG scanline filter")


def validate_icon(path):
    data = contract.regular(path, MAX_ICON_BYTES).read_bytes()
    if (
        len(data) < 8
        or data[:4] != b"icns"
        or struct.unpack_from(">I", data, 4)[0] != len(data)
    ):
        raise ValueError("Invalid ICNS container")
    offset = 8
    seen = set()
    records = []
    table = None
    while offset < len(data):
        if len(data) - offset < 8:
            raise ValueError("Truncated ICNS entry")
        kind, size = struct.unpack_from(">4sI", data, offset)
        if kind in seen or size <= 8 or offset + size > len(data):
            raise ValueError("Duplicate or truncated ICNS entry")
        body = data[offset + 8:offset + size]
        if kind == b"TOC ":
            if seen or len(body) % 8:
                raise ValueError("ICNS TOC must be first and contain complete records")
            table = list(struct.iter_unpack(">4sI", body))
        elif kind in ICNS_SIZES:
            validate_png(body, ICNS_SIZES[kind])
            records.append((kind, size))
        else:
            raise ValueError("Unsupported ICNS entry")
        seen.add(kind)
        offset += size
    if not records:
        raise ValueError("ICNS requires at least one real image")
    if table is not None and table != records:
        raise ValueError("ICNS TOC differs from actual image entries")


def payload(version, target, binary, notices, icon):
    executable = executable_path(target)
    contract.regular(binary, contract.MAX_EXECUTABLE_BYTES)
    with binary.open("rb") as stream:
        contract.architecture(stream, binary.stat().st_size, target)
    contract.regular(notices, contract.MAX_LEGAL_BYTES)
    if not notices.read_text(encoding="utf-8").strip():
        raise ValueError("Desktop notices must not be blank")
    files = {
        executable: binary,
        resource_path(target, "LICENSE"): contract.regular(
            contract.ROOT / "LICENSE", contract.MAX_LEGAL_BYTES,
        ),
        resource_path(target, NOTICES): notices,
    }
    if "apple" in target:
        if icon is None:
            raise ValueError("macOS requires an explicit reviewed ICNS icon")
        validate_icon(icon)
        files[resource_path(target, "TableCraft.icns")] = icon
        files["TableCraft.app/Contents/Info.plist"] = plistlib.dumps(bundle_metadata(version), sort_keys=True)
    elif icon is not None:
        raise ValueError("This portable target does not consume an external ICNS icon")
    return files


def digest(value):
    return contract.digest(value) if isinstance(value, Path) else hashlib.sha256(value).hexdigest()


def manifest(version, target, files):
    metadata = {
        "product": PRODUCT,
        "version": version,
        "target": target,
        "files": {name: digest(value) for name, value in sorted(files.items())},
    }
    return (json.dumps(metadata, indent=2) + "\n").encode("utf-8")


def package(version, target, binary, notices, output, icon=None):
    contract.validate_version(version)
    files = payload(version, target, binary, notices, icon)
    files[resource_path(target, MANIFEST)] = manifest(version, target, files)
    if output.is_symlink():
        raise ValueError("Output directory must not be a symlink")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / archive_name(version, target)
    sidecar = archive.with_name(archive.name + ".sha256")
    if archive.exists() or archive.is_symlink() or sidecar.exists() or sidecar.is_symlink():
        raise ValueError("Desktop archive and checksum require fresh paths")
    created_archive = False
    created_sidecar = False
    try:
        with archive.open("xb") as raw:
            created_archive = True
            if archive.suffix == ".zip":
                with zipfile.ZipFile(raw, "w", zipfile.ZIP_DEFLATED) as stream:
                    for name, value in sorted(files.items()):
                        entry = zipfile.ZipInfo(name, ZIP_EPOCH)
                        entry.create_system = 3
                        mode = EXECUTABLE_MODE if name == executable_path(target) else DATA_MODE
                        entry.external_attr = (stat.S_IFREG | mode) << 16
                        entry.compress_type = zipfile.ZIP_DEFLATED
                        with stream.open(entry, "w", force_zip64=True) as destination:
                            if isinstance(value, Path):
                                with value.open("rb") as source:
                                    shutil.copyfileobj(source, destination, COPY_BYTES)
                            else:
                                destination.write(value)
            else:
                with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, filename="") as zipped:
                    with tarfile.open(fileobj=zipped, mode="w") as stream:
                        for name, value in sorted(files.items()):
                            entry = tarfile.TarInfo(name)
                            entry.mode = EXECUTABLE_MODE if name == executable_path(target) else DATA_MODE
                            entry.size = value.stat().st_size if isinstance(value, Path) else len(value)
                            if isinstance(value, Path):
                                with value.open("rb") as source:
                                    stream.addfile(entry, source)
                            else:
                                stream.addfile(entry, io.BytesIO(value))
        verify_archive(archive, version, target, notices, icon)
        with sidecar.open("xb") as stream:
            created_sidecar = True
            stream.write(f"{contract.digest(archive)}  {archive.name}\n".encode("ascii"))
    except BaseException:
        if created_archive:
            archive.unlink(missing_ok=True)
        if created_sidecar:
            sidecar.unlink(missing_ok=True)
        raise
    return archive


def smoke(binary, version):
    for argument in ("--version", "--help"):
        result = subprocess.run([str(binary.resolve()), argument], capture_output=True,
                                text=True, encoding="utf-8", timeout=30)
        if result.returncode or result.stderr:
            raise ValueError(f"Extracted desktop {argument} failed")
        if argument == "--version" and result.stdout.strip() != f"tablecraft-desktop {version}":
            raise ValueError("Desktop version mismatch")
        if argument == "--help" and ("Usage:" not in result.stdout or "[DATABASE]" not in result.stdout):
            raise ValueError("Desktop help is incomplete")


def verify_archive(archive, version, target, notices, icon=None, run_smoke=False):
    contract.validate_version(version)
    executable = executable_path(target)
    # Validate caller-owned legal/icon inputs without treating the archive as authority.
    references = {
        resource_path(target, "LICENSE"): contract.regular(
            contract.ROOT / "LICENSE", contract.MAX_LEGAL_BYTES,
        ),
        resource_path(target, NOTICES): contract.regular(notices, contract.MAX_LEGAL_BYTES),
    }
    if not notices.read_text(encoding="utf-8").strip():
        raise ValueError("Desktop notices must not be blank")
    manifest_name = resource_path(target, MANIFEST)
    expected = {executable, *references, manifest_name}
    if "apple" in target:
        if icon is None:
            raise ValueError("macOS verification requires the reviewed icon")
        validate_icon(icon)
        references[resource_path(target, "TableCraft.icns")] = icon
        expected.update((resource_path(target, "TableCraft.icns"), "TableCraft.app/Contents/Info.plist"))
    elif icon is not None:
        raise ValueError("Unexpected icon for portable target")
    contract.regular(archive, MAX_ARCHIVE_BYTES)
    if archive.name != archive_name(version, target):
        raise ValueError("Unexpected desktop archive name")
    with tempfile.TemporaryDirectory(prefix="tablecraft-desktop-verify-") as temporary:
        directory = Path(temporary)
        hashes = {}

        def extract(name, source, size, mode):
            if name not in expected or name in hashes:
                raise ValueError("Unexpected or duplicate desktop archive member")
            if name == executable:
                maximum = contract.MAX_EXECUTABLE_BYTES
            elif name.endswith(".icns"):
                maximum = MAX_ICON_BYTES
            elif name == manifest_name or name.endswith(".plist"):
                maximum = MAX_MANIFEST_BYTES
            else:
                maximum = contract.MAX_LEGAL_BYTES
            expected_mode = EXECUTABLE_MODE if name == executable else DATA_MODE
            if size <= 0 or size > maximum or mode != expected_mode:
                raise ValueError("Invalid desktop member size or permissions")
            destination = directory / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            count = 0
            with destination.open("xb") as output:
                while block := source.read(min(COPY_BYTES, maximum - count + 1)):
                    count += len(block)
                    if count > maximum:
                        raise ValueError("Oversized desktop archive member")
                    output.write(block)
            if count != size:
                raise ValueError("Desktop member length mismatch")
            destination.chmod(mode)
            hashes[name] = contract.digest(destination)

        if archive.suffix == ".zip":
            with zipfile.ZipFile(archive) as stream:
                for entry in stream.infolist():
                    mode = entry.external_attr >> 16
                    if entry.is_dir() or stat.S_IFMT(mode) != stat.S_IFREG or entry.flag_bits & 1:
                        raise ValueError("Only unencrypted regular ZIP members are admitted")
                    with stream.open(entry) as source:
                        extract(entry.filename, source, entry.file_size, stat.S_IMODE(mode))
        else:
            with tarfile.open(archive, "r:gz") as stream:
                for entry in stream:
                    if not entry.isfile() or entry.issparse():
                        raise ValueError("Only regular nonsparse tar members are admitted")
                    with stream.extractfile(entry) as source:
                        extract(entry.name, source, entry.size, entry.mode)
        if set(hashes) != expected:
            raise ValueError("Incomplete desktop payload")
        metadata = json.loads(
            (directory / manifest_name).read_text(encoding="utf-8"),
            object_pairs_hook=unique_members,
        )
        hashes.pop(manifest_name)
        expected_metadata = {
            "product": PRODUCT, "version": version, "target": target, "files": hashes,
        }
        if metadata != expected_metadata:
            raise ValueError("Desktop manifest or payload hashes differ")
        for name, reference in references.items():
            if hashes[name] != contract.digest(reference):
                raise ValueError("Desktop legal/icon payload differs from reviewed input")
        if "apple" in target:
            info = (directory / "TableCraft.app/Contents/Info.plist").read_bytes()
            if info != plistlib.dumps(bundle_metadata(version), sort_keys=True):
                raise ValueError("Unexpected desktop bundle metadata")
        binary = directory / executable
        with binary.open("rb") as stream:
            contract.architecture(stream, binary.stat().st_size, target)
        if run_smoke:
            smoke(binary, version)


def unique_members(pairs):
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError("Duplicate desktop manifest key")
        result[name] = value
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("package", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--version", required=True)
        command.add_argument("--target", choices=contract.TARGETS, required=True)
        command.add_argument("--notices", type=Path, help="Explicit reviewed bundle; default: generate from locked sources")
        command.add_argument("--icon", type=Path)
        if name == "package":
            command.add_argument("--binary", type=Path, required=True)
            command.add_argument("--output", type=Path, required=True)
        else:
            command.add_argument("--archive", type=Path, required=True)
            command.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    try:
        with desktop_notices.bundle(args.notices) as notices:
            if args.command == "package":
                print(package(args.version, args.target, args.binary, notices, args.output, args.icon))
            else:
                verify_archive(args.archive, args.version, args.target, notices, args.icon, args.smoke)
    except (
        OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, zipfile.BadZipFile,
        tarfile.TarError, zlib.error,
    ) as error:
        parser.exit(1, f"Desktop packaging failed: {error}\n")


if __name__ == "__main__":
    main()
