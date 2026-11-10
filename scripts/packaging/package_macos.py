#!/usr/bin/env python3
"""Build and headlessly verify a branded DMG from an already checked desktop ZIP."""
import argparse
from contextlib import contextmanager
import hashlib
import os
from pathlib import Path
import plistlib
import runpy
import stat
import subprocess
import sys
import tempfile
import zipfile

import package_desktop as desktop
import installer_contract as installers
import release_contract as contract

MACOS = Path(__file__).resolve().parents[2] / "packaging/macos"
TARGETS = ("x86_64-apple-darwin", "aarch64-apple-darwin")
APP = "TableCraft.app"
BACKGROUND = ".background.png"
VOLUME_ICON = ".VolumeIcon.icns"
STORE = ".DS_Store"
COPY_BYTES = 1024 * 1024
MAX_STORE_BYTES = 1024 * 1024
HDIUTIL = "/usr/bin/hdiutil"
APP_FILES = {
    "TableCraft.app/Contents/MacOS/tablecraft-desktop",
    "TableCraft.app/Contents/Info.plist",
    "TableCraft.app/Contents/Resources/LICENSE",
    "TableCraft.app/Contents/Resources/ThirdPartyLicenses.txt",
    "TableCraft.app/Contents/Resources/TableCraft.icns",
    "TableCraft.app/Contents/Resources/RELEASE.json",
}


def artwork(destination):
    runpy.run_path(str(MACOS / "background.py"))["render"](destination)


def checked_source(archive, version, target, notices, icon):
    if target not in TARGETS:
        raise ValueError("DMG packages require a supported macOS target")
    desktop.verify_archive(archive, version, target, notices, icon)
    return contract.digest(archive)


def extract_checked(archive, directory):
    """The prior full verifier owns archive admission; repeat safe regular-file framing."""
    with zipfile.ZipFile(archive) as source:
        entries = source.infolist()
        if len(entries) != len(APP_FILES) or {entry.filename for entry in entries} != APP_FILES:
            raise ValueError("DMG source does not contain exactly its checked application")
        for entry in entries:
            name = entry.filename
            mode = entry.external_attr >> 16
            if (
                not name.startswith(APP + "/")
                or any(part in ("", ".", "..") for part in name.split("/"))
                or "\\" in name or ":" in name or "\0" in name
                or stat.S_IFMT(mode) != stat.S_IFREG
                or entry.is_dir() or entry.flag_bits & 1
                or entry.file_size <= 0
            ):
                raise ValueError("Invalid checked DMG application member")
            if name.endswith("tablecraft-desktop"):
                maximum = contract.MAX_EXECUTABLE_BYTES
            elif name.endswith(".icns"):
                maximum = desktop.MAX_ICON_BYTES
            elif name.endswith(".json") or name.endswith(".plist"):
                maximum = desktop.MAX_MANIFEST_BYTES
            else:
                maximum = contract.MAX_LEGAL_BYTES
            if entry.file_size > maximum:
                raise ValueError("DMG source member exceeds its resource allowance")
            destination = directory / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            with source.open(entry) as stream, destination.open("xb") as output:
                count = 0
                while block := stream.read(COPY_BYTES):
                    count += len(block)
                    if count > entry.file_size:
                        raise ValueError("DMG source member exceeds its checked size")
                    output.write(block)
                if count != entry.file_size:
                    raise ValueError("DMG source member length changed")
            destination.chmod(stat.S_IMODE(mode))
    return directory / APP


def build_disk_image(app, image, icon, background):
    if sys.platform != "darwin":
        raise ValueError("DMG construction requires macOS")
    try:
        import dmgbuild
        from dmgbuild import core
    except ImportError as error:
        raise ValueError("DMG construction requires packaging/macos/requirements.txt") from error

    layout = runpy.run_path(str(MACOS / "dmg_settings.py"))
    original_hdiutil = core.hdiutil

    def headless_hdiutil(command, *arguments, **options):
        if command == "attach":
            arguments = ("-noautoopen", *arguments)
        return original_hdiutil(command, *arguments, **options)

    # dmgbuild writes DS_Store directly, without Finder or AppleScript. Its own
    # writable staging mount also receives the explicit no-auto-open restriction.
    core.hdiutil = headless_hdiutil
    try:
        dmgbuild.build_dmg(
            str(image), layout["VOLUME_NAME"],
            settings=layout["settings"](app, icon, background),
        )
    finally:
        core.hdiutil = original_hdiutil


def hdiutil(*arguments):
    return subprocess.run(
        [HDIUTIL, *map(str, arguments)],
        capture_output=True, check=True, timeout=240,
    )


@contextmanager
def mounted(image, parent):
    mount = parent / "volume"
    mount.mkdir()
    attached = False
    try:
        result = hdiutil(
            "attach", "-readonly", "-nobrowse", "-noautoopen",
            "-mountpoint", mount, "-plist", image,
        )
        attached = True
        metadata = plistlib.loads(result.stdout)
        mounts = [entry for entry in metadata.get("system-entities", []) if entry.get("mount-point")]
        if len(mounts) != 1 or Path(mounts[0]["mount-point"]).resolve() != mount.resolve():
            raise ValueError("DMG mounted outside its owned verification directory")
        yield mount
    finally:
        # A failed attach may still have mounted this previously empty owned path.
        # Never detach another volume or leave our mount behind on a check failure.
        if attached or os.path.ismount(mount):
            hdiutil("detach", "-quiet", mount)


def validate_store(path):
    try:
        from ds_store import DSStore
    except ImportError as error:
        raise ValueError("DMG verification requires packaging/macos/requirements.txt") from error
    contract.regular(path, MAX_STORE_BYTES)
    layout = runpy.run_path(str(MACOS / "dmg_settings.py"))
    with DSStore.open(str(path), "r") as store:
        for name, position in layout["ICON_LOCATIONS"].items():
            if store[name]["Iloc"] != position:
                raise ValueError("DMG icon placement differs from the branded layout")
        view = store["."]["icvp"]
        if view["iconSize"] != layout["ICON_SIZE"] or view["backgroundType"] != 2:
            raise ValueError("DMG icon view or custom background differs")
        window = store["."]["bwsp"]
        (x, y), (width, height) = layout["WINDOW_RECT"]
        bounds = f"{{{{{x}, {y}}}, {{{width}, {height}}}}}"
        if window["WindowBounds"] != bounds or any(
            window[key] for key in ("ShowStatusBar", "ShowToolbar", "ShowPathbar", "ShowSidebar", "ShowTabView")
        ):
            raise ValueError("DMG chrome differs from the branded layout")


def validate_mounted(volume, archive, icon, background):
    expected_top = {APP, "Applications", BACKGROUND, STORE, VOLUME_ICON}
    if {path.name for path in volume.iterdir()} != expected_top:
        raise ValueError("DMG volume contains missing or unlisted members")
    applications = volume / "Applications"
    if not applications.is_symlink() or os.readlink(applications) != "/Applications":
        raise ValueError("DMG Applications link differs")
    if contract.digest(contract.regular(volume / BACKGROUND)) != contract.digest(background):
        raise ValueError("DMG custom background differs")
    if contract.digest(contract.regular(volume / VOLUME_ICON)) != contract.digest(icon):
        raise ValueError("DMG volume icon differs")
    validate_store(volume / STORE)

    if not stat.S_ISDIR((volume / APP).lstat().st_mode):
        raise ValueError("DMG application must be a real directory")
    with zipfile.ZipFile(archive) as source:
        expected = {entry.filename: entry for entry in source.infolist()}
        expected_directories = {
            str(parent)
            for name in expected
            for parent in Path(name).parents
            if str(parent) != "."
        }
        actual = set()
        for directory, directories, files in os.walk(volume / APP, followlinks=False):
            if Path(directory).relative_to(volume).as_posix() not in expected_directories:
                raise ValueError("DMG application contains an unlisted directory")
            for name in directories:
                if (Path(directory) / name).is_symlink():
                    raise ValueError("DMG application contains a directory symlink")
            for name in files:
                path = Path(directory) / name
                relative = path.relative_to(volume).as_posix()
                if relative not in expected:
                    raise ValueError("DMG application contains an unlisted file")
                metadata = path.lstat()
                entry = expected[relative]
                if not stat.S_ISREG(metadata.st_mode) or metadata.st_size != entry.file_size:
                    raise ValueError("DMG application member type or size differs")
                if stat.S_IMODE(metadata.st_mode) != stat.S_IMODE(entry.external_attr >> 16):
                    raise ValueError("DMG application member permissions differ")
                with source.open(entry) as stream:
                    digest = hashlib.file_digest(stream, "sha256").hexdigest()
                if contract.digest(path) != digest:
                    raise ValueError("DMG application differs from its checked update ZIP")
                actual.add(relative)
        if actual != set(expected):
            raise ValueError("DMG application is incomplete")


def verify(image, archive, version, target, notices, icon):
    if sys.platform != "darwin":
        raise ValueError("DMG verification requires macOS")
    fingerprint = checked_source(archive, version, target, notices, icon)
    installers.verify_container(image, version, target, "dmg")
    hdiutil("verify", "-quiet", image)
    information = plistlib.loads(hdiutil("imageinfo", "-plist", image).stdout)
    properties = information.get("Properties", {})
    if (
        information.get("Format") != "UDZO"
        or properties.get("Compressed") is not True
        or properties.get("Checksummed") is not True
        or properties.get("Encrypted") is not False
    ):
        raise ValueError("DMG must use the checksummed, compressed, unencrypted release format")
    with tempfile.TemporaryDirectory(prefix="tablecraft-dmg-verify-") as temporary:
        directory = Path(temporary)
        background = directory / "background.png"
        artwork(background)
        with mounted(image, directory) as volume:
            validate_mounted(volume, archive, icon, background)
    if contract.digest(archive) != fingerprint:
        raise ValueError("DMG source archive changed during verification")


def package(archive, version, target, notices, icon, output):
    fingerprint = checked_source(archive, version, target, notices, icon)
    name = installers.name(version, target, "dmg")
    if output.is_symlink():
        raise ValueError("DMG output directory must not be a symlink")
    output.mkdir(parents=True, exist_ok=True)
    destination = output / name
    sidecar = destination.with_name(destination.name + ".sha256")
    if destination.exists() or destination.is_symlink() or sidecar.exists() or sidecar.is_symlink():
        raise ValueError("DMG and checksum require fresh paths")
    created = []
    with tempfile.TemporaryDirectory(prefix=".tablecraft-dmg-", dir=output) as temporary:
        root = Path(temporary)
        app = extract_checked(archive, root / "payload")
        background = root / "background.png"
        artwork(background)
        image = root / name
        build_disk_image(app, image, icon, background)
        verify(image, archive, version, target, notices, icon)
        if contract.digest(archive) != fingerprint:
            raise ValueError("DMG source archive changed during construction")
        try:
            # The owned staging file is already verified. A same-filesystem hard
            # link publishes it without replacing another process's output.
            destination.hardlink_to(image)
            created.append(destination)
            with sidecar.open("xb") as stream:
                created.append(sidecar)
                stream.write(installers.checksum(destination))
        except BaseException:
            for path in reversed(created):
                path.unlink(missing_ok=True)
            raise
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for name in ("package", "verify"):
        command = commands.add_parser(name)
        command.add_argument("--version", required=True)
        command.add_argument("--target", choices=TARGETS, required=True)
        command.add_argument("--archive", type=Path, required=True, help="Checked desktop update ZIP")
        command.add_argument("--notices", type=Path, required=True)
        command.add_argument("--icon", type=Path, required=True)
        if name == "package":
            command.add_argument("--output", type=Path, required=True)
        else:
            command.add_argument("--dmg", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "package":
            print(package(args.archive, args.version, args.target, args.notices, args.icon, args.output))
        else:
            verify(args.dmg, args.archive, args.version, args.target, args.notices, args.icon)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        parser.exit(1, f"macOS packaging failed: {error}\n")


if __name__ == "__main__":
    main()
