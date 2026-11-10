#!/usr/bin/env python3
"""Build native desktop DEB, RPM and AppImage packages without installing them.

Use native Linux tools on the matching architecture. AppImage dependency/RPATH
handling belongs to linuxdeploy, not a custom ELF loader. Tool/runtime downloads
are deliberately external inputs so packaging cannot fetch an unreviewed runtime.
"""
import argparse
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile

import file_associations
import installer_contract as installers
import package_desktop as desktop
import release_contract as contract

PRODUCT = "tablecraft-desktop"
BINARY = "tablecraft-desktop"
APPLICATION_ID = file_associations.APPLICATION_ID
ASSETS = contract.ROOT / "packaging/linux"
LICENSE_DIRECTORY = Path("usr/share/licenses/tablecraft")
SYSTEM_PAYLOAD = Path("usr/lib/tablecraft")
APPIMAGE_PAYLOAD = Path("usr/bin")
ICON_SIZE = 256
MAX_TOOL_OUTPUT_BYTES = 4 * 1024 * 1024
MAX_RUNTIME_SOURCE_MEMBERS = 4096
TARGETS = {
    "x86_64-unknown-linux-gnu": ("x86_64", "amd64", "x86-64"),
    "aarch64-unknown-linux-gnu": ("aarch64", "arm64", "AArch64"),
}
# These are loaded dynamically by the enabled X11/Wayland/font backends and are
# not all discoverable from the main executable's DT_NEEDED entries.
DLOPEN_LIBRARIES = (
    "libX11.so.6", "libX11-xcb.so.1", "libXcursor.so.1", "libXi.so.6", "libXrandr.so.2",
    "libxcb.so.1", "libxcb-render.so.0", "libxcb-shape.so.0", "libxcb-xfixes.so.0",
    "libxkbcommon.so.0", "libxkbcommon-x11.so.0", "libwayland-client.so.0", "libwayland-cursor.so.0",
    "libwayland-egl.so.1", "libfontconfig.so.1",
)
DEBIAN_RUNTIME_DEPENDENCIES = (
    "desktop-file-utils", "shared-mime-info", "libx11-6", "libx11-xcb1", "libxcursor1", "libxi6",
    "libxrandr2", "libxcb1", "libxcb-render0", "libxcb-shape0", "libxcb-xfixes0",
    "libxkbcommon0", "libxkbcommon-x11-0", "libwayland-client0", "libwayland-cursor0", "libwayland-egl1",
    "libegl1", "libgl1", "libfontconfig1",
)
PACKAGE_NAME = re.compile(r"[a-z0-9][a-z0-9+.-]*(?::[a-z0-9-]+)?\Z")
COMMON_LICENSE = re.compile(r"/usr/share/common-licenses/([A-Za-z0-9.+-]+)")


@dataclass(frozen=True)
class Inputs:
    version: str
    target: str
    binary: Path
    notices: Path
    icon: Path


@dataclass(frozen=True)
class AppImageTools:
    linuxdeploy: Path
    appimagetool: Path
    runtime: Path
    runtime_notices: Path
    runtime_source: Path


class ToolFailure(ValueError):
    """A native tool failure with bounded diagnostic context."""


def tool_environment(environment=None):
    environment = dict(os.environ if environment is None else environment)
    environment.pop("TABLECRAFT_UPDATE_SIGNING_KEY", None)
    environment.pop("TABLECRAFT_UPDATE_SIGNING_PASSWORD", None)
    return environment


def run(command, *, directory=None, environment=None):
    result = subprocess.run(
        [str(value) for value in command], cwd=directory, env=tool_environment(environment),
        stdin=subprocess.DEVNULL,
        capture_output=True, check=False, timeout=600,
    )
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace")[:4096]
        raise ToolFailure(f"Linux packaging tool failed: {Path(command[0]).name} ({result.returncode})\n{detail}")
    if len(result.stdout) > MAX_TOOL_OUTPUT_BYTES:
        raise ValueError("Linux packaging tool output exceeds its bound")
    return result.stdout.decode("utf-8")


def validate_inputs(inputs):
    contract.validate_version(inputs.version)
    if inputs.target not in TARGETS:
        raise ValueError("A supported native Linux target is required")
    machine = platform.machine().lower()
    expected = TARGETS[inputs.target][0]
    if platform.system() != "Linux" or machine != expected:
        raise ValueError("Linux installers require the matching native Linux host")
    desktop.payload(inputs.version, inputs.target, inputs.binary, inputs.notices, None)
    contract.regular(inputs.icon, desktop.MAX_ICON_BYTES)
    desktop.validate_png(inputs.icon.read_bytes(), ICON_SIZE)


def write(path, contents, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("xb") as stream:
        stream.write(contents)
    path.chmod(mode)


def copy(source, destination, mode=0o644):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("xb") as target, source.open("rb") as original:
        shutil.copyfileobj(original, target, desktop.COPY_BYTES)
    destination.chmod(mode)


def release_files(directory, inputs):
    files = {
        BINARY: directory / BINARY,
        "LICENSE": directory / "LICENSE",
        desktop.NOTICES: directory / desktop.NOTICES,
    }
    manifest = directory / desktop.MANIFEST
    if manifest.exists():
        manifest.unlink()
    write(manifest, desktop.manifest(inputs.version, inputs.target, files))


def stage(root, inputs, *, appimage=False):
    payload = root / (APPIMAGE_PAYLOAD if appimage else SYSTEM_PAYLOAD)
    copy(inputs.binary, payload / BINARY, 0o755)
    copy(contract.ROOT / "LICENSE", payload / "LICENSE")
    copy(inputs.notices, payload / desktop.NOTICES)
    release_files(payload, inputs)
    copy(contract.ROOT / "LICENSE", root / LICENSE_DIRECTORY / "LICENSE")
    copy(inputs.notices, root / LICENSE_DIRECTORY / desktop.NOTICES)

    associations = file_associations.linux_payloads("/usr/bin/tablecraft-desktop")
    desktop_name = "applications/" + APPLICATION_ID + ".desktop"
    entry = associations[desktop_name].decode("utf-8")
    if appimage:
        entry = "\n".join(
            "Exec=tablecraft-desktop -- %f" if line.startswith("Exec=") else line
            for line in entry.splitlines()
        ) + "\n"
    entry += f"Icon={APPLICATION_ID}\nStartupNotify=true\n"
    associations[desktop_name] = entry.encode("utf-8")
    for name, contents in associations.items():
        write(root / "usr/share" / name, contents)
    copy(inputs.icon, root / f"usr/share/icons/hicolor/{ICON_SIZE}x{ICON_SIZE}/apps/{APPLICATION_ID}.png")
    copy(ASSETS / (APPLICATION_ID + ".metainfo.xml"), root / f"usr/share/metainfo/{APPLICATION_ID}.metainfo.xml")
    if not appimage:
        launcher = root / "usr/bin" / BINARY
        launcher.parent.mkdir(parents=True, exist_ok=True)
        launcher.symlink_to("../lib/tablecraft/" + BINARY)
    return payload


def package_version(version):
    return version.replace("-", "~", 1)


def debian_dependencies(inputs, temporary):
    debian = temporary / "debian"
    debian.mkdir()
    write(debian / "control", b"Source: tablecraft-desktop\nSection: database\nPriority: optional\nMaintainer: Replicraft\n\nPackage: tablecraft-desktop\nArchitecture: any\nDescription: Access-compatible database workspace\n")
    output = run(["dpkg-shlibdeps", "-O", "-e", inputs.binary.resolve()], directory=temporary)
    lines = output.splitlines()
    if len(lines) != 1 or not lines[0].startswith("shlibs:Depends="):
        raise ValueError("dpkg-shlibdeps did not provide one dependency set")
    linked = lines[0].removeprefix("shlibs:Depends=").strip()
    if not linked or any(character in linked for character in "\r\n\0"):
        raise ValueError("Invalid native Debian dependency set")
    return ", ".join((linked, *DEBIAN_RUNTIME_DEPENDENCIES))


def build_deb(inputs, root, output, temporary):
    dependencies = debian_dependencies(inputs, temporary)
    installed_kib = (sum(path.stat().st_size for path in root.rglob("*") if path.is_file() and not path.is_symlink()) + 1023) // 1024
    control = (
        "Package: tablecraft-desktop\n"
        f"Version: {package_version(inputs.version)}\n"
        f"Architecture: {TARGETS[inputs.target][1]}\n"
        "Maintainer: Replicraft\n"
        "Section: database\nPriority: optional\n"
        f"Installed-Size: {installed_kib}\n"
        f"Depends: {dependencies}\n"
        "Recommends: xdg-desktop-portal, xdg-desktop-portal-gtk | xdg-desktop-portal-kde\n"
        "Homepage: https://github.com/replicraft/TableCraft\n"
        "Description: Access-compatible database workspace\n"
        " Browse and edit MDB and ACCDB databases with native Access semantics.\n"
        " Compatibility and editing are still in development.\n"
    )
    write(root / "DEBIAN/control", control.encode("utf-8"))
    copy(ASSETS / "refresh-caches.sh", root / "DEBIAN/postinst", 0o755)
    copy(ASSETS / "refresh-caches.sh", root / "DEBIAN/postrm", 0o755)
    run(["dpkg-deb", "--root-owner-group", "--build", root, output])
    fields = run(["dpkg-deb", "--field", output, "Package", "Version", "Architecture"])
    for expected in ("Package: tablecraft-desktop", f"Version: {package_version(inputs.version)}", f"Architecture: {TARGETS[inputs.target][1]}"):
        if expected not in fields.splitlines():
            raise ValueError("DEB product/version/architecture mismatch")
    verify_deb_files(output, root)


def build_rpm(inputs, root, output, temporary):
    for directory in ("BUILD", "RPMS", "SOURCES", "SPECS", "SRPMS", "BUILDROOT"):
        (temporary / directory).mkdir()
    source = temporary / "SOURCES/payload"
    shutil.copytree(root, source, symlinks=True)
    required = ", ".join(name + "()(64bit)" for name in (*DLOPEN_LIBRARIES, "libEGL.so.1", "libGL.so.1"))
    caches = (ASSETS / "refresh-caches.sh").read_text().splitlines()[1:]
    spec = (
        "%global debug_package %{nil}\n%global __strip /bin/true\n%global _build_id_links none\n"
        "Name: tablecraft-desktop\n"
        f"Version: {package_version(inputs.version)}\nRelease: 1\n"
        "Summary: Access-compatible database workspace\nLicense: Apache-2.0\n"
        "URL: https://github.com/replicraft/TableCraft\n"
        f"BuildArch: {TARGETS[inputs.target][0]}\n"
        f"Requires: {required}\nRequires: desktop-file-utils, shared-mime-info\n"
        "AutoReqProv: yes\n\n%description\n"
        "Browse and edit Microsoft Access databases. Compatibility is in development.\n\n"
        "%install\nmkdir -p \"%{buildroot}\"\ncp -a \"%{_sourcedir}/payload/.\" \"%{buildroot}/\"\n\n"
        "%post\n" + "\n".join(caches) + "\n\n%postun\n" + "\n".join(caches) + "\n\n"
        "%files\n%defattr(-,root,root,-)\n/usr/bin/tablecraft-desktop\n/usr/lib/tablecraft\n"
        "/usr/share/applications/org.replicraft.tablecraft.desktop\n"
        "/usr/share/mime/packages/org.replicraft.tablecraft.xml\n"
        "/usr/share/icons/hicolor/256x256/apps/org.replicraft.tablecraft.png\n"
        "/usr/share/metainfo/org.replicraft.tablecraft.metainfo.xml\n"
        "%license /usr/share/licenses/tablecraft\n"
    )
    spec_path = temporary / "SPECS/tablecraft.spec"
    write(spec_path, spec.encode("utf-8"))
    run(["rpmbuild", "--define", f"_topdir {temporary.resolve()}", "--define", "_binary_filedigest_algorithm 8", "-bb", spec_path])
    artifacts = list((temporary / "RPMS").rglob("*.rpm"))
    if len(artifacts) != 1:
        raise ValueError("rpmbuild did not produce exactly one package")
    shutil.copyfile(artifacts[0], output)
    identity = run(["rpm", "-qp", "--queryformat", "%{NAME}\\n%{VERSION}\\n%{ARCH}\\n", output]).splitlines()
    if identity != [PRODUCT, package_version(inputs.version), TARGETS[inputs.target][0]]:
        raise ValueError("RPM product/version/architecture mismatch")
    verify_rpm_files(output, root)


def inventory(root):
    files = {}
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if relative.parts[0] == "DEBIAN":
            continue
        info = path.lstat()
        name = relative.as_posix()
        if stat.S_ISLNK(info.st_mode):
            if not path.resolve().is_relative_to(root.resolve()):
                raise ValueError("An installed package link escapes its owned payload")
            files[name] = ("link", os.readlink(path))
        elif stat.S_ISREG(info.st_mode):
            files[name] = ("file", info.st_size, stat.S_IMODE(info.st_mode), contract.digest(path))
        elif not stat.S_ISDIR(info.st_mode):
            raise ValueError("Native package payload contains a special file")
    return files


def parent_directories(files):
    directories = {"", "."}
    for name in files:
        directories.update(parent.as_posix() for parent in Path(name).parents if parent.as_posix() != ".")
    return directories


def verify_deb_files(package, root):
    expected = inventory(root)
    directories = parent_directories(expected)
    observed = set()
    with tempfile.TemporaryFile() as errors:
        process = subprocess.Popen(["dpkg-deb", "--fsys-tarfile", str(package)], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=errors, env=tool_environment())
        try:
            if process.stdout is None:
                raise ValueError("Cannot read DEB payload stream")
            with tarfile.open(fileobj=process.stdout, mode="r|") as stream:
                entries = 0
                for member in stream:
                    entries += 1
                    if entries > len(expected) + len(directories):
                        raise ValueError("DEB payload has too many entries")
                    name = member.name.removeprefix("./").removesuffix("/")
                    if member.isdir() and name in directories:
                        continue
                    record = expected.get(name)
                    if record is None or name in observed:
                        raise ValueError("DEB payload membership differs from staging")
                    observed.add(name)
                    if record[0] == "link":
                        if not member.issym() or member.linkname != record[1]:
                            raise ValueError("DEB launcher link differs")
                        continue
                    if not member.isfile() or member.size != record[1] or member.mode != record[2]:
                        raise ValueError("DEB payload type, size or permissions differ")
                    source = stream.extractfile(member)
                    if source is None:
                        raise ValueError("Cannot read a DEB regular-file member")
                    digest = hashlib.file_digest(source, "sha256").hexdigest()
                    if digest != record[3]:
                        raise ValueError("DEB payload bytes differ from staging")
            if process.wait(timeout=600) != 0 or observed != set(expected):
                raise ValueError("DEB payload is incomplete")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            if process.stdout is not None:
                process.stdout.close()


def verify_rpm_files(package, root):
    expected = inventory(root)
    directories = parent_directories(expected)
    output = run(["rpm", "-qp", "--queryformat", "[%{FILENAMES}\\t%{FILEMODES}\\t%{FILELINKTOS}\\t%{FILEDIGESTS}\\n]", package])
    observed = set()
    for line in output.splitlines():
        parts = line.split("\t")
        if len(parts) != 4 or not parts[0].startswith("/"):
            raise ValueError("Invalid RPM file inventory")
        name, mode, target, digest = parts
        name = name.removeprefix("/")
        mode = int(mode)
        if stat.S_ISDIR(mode) and name in directories:
            continue
        record = expected.get(name)
        if record is None or name in observed:
            raise ValueError("RPM payload membership differs from staging")
        observed.add(name)
        if record[0] == "link":
            if not stat.S_ISLNK(mode) or target != record[1]:
                raise ValueError("RPM launcher link differs")
        elif not stat.S_ISREG(mode) or stat.S_IMODE(mode) != record[2] or digest != record[3]:
            raise ValueError("RPM payload type, mode or SHA-256 differs")
    if observed != set(expected):
        raise ValueError("RPM payload is incomplete")


def verify_appimage(package, root, inputs, temporary):
    # Execute only the newly built native container's extraction operation. This
    # does not install it, open a database or launch the graphical application.
    destination = temporary / "appimage-inspection"
    destination.mkdir()
    environment = os.environ.copy()
    for key in ("APPDIR", "APPIMAGE", "ARGV0", "OWD"):
        environment.pop(key, None)
    run([package.resolve(), "--appimage-extract"], directory=destination, environment=environment)
    extracted = destination / "squashfs-root"
    if not extracted.is_dir() or inventory(extracted) != inventory(root):
        raise ValueError("AppImage extracted payload differs from staging")
    payload = extracted / APPIMAGE_PAYLOAD
    metadata = json.loads((payload / desktop.MANIFEST).read_text("utf-8"))
    files = {name: payload / name for name in (BINARY, "LICENSE", desktop.NOTICES)}
    if (payload / desktop.MANIFEST).read_bytes() != desktop.manifest(inputs.version, inputs.target, files):
        raise ValueError("AppImage release identity or payload hashes differ")
    if metadata["product"] != PRODUCT:
        raise ValueError("AppImage product identity differs")
    desktop.smoke(payload / BINARY, inputs.version)



def library_sources(target):
    output = run(["ldconfig", "-p"])
    marker = TARGETS[target][2]
    libraries = {}
    for line in output.splitlines():
        if " => " not in line or marker not in line:
            continue
        name, path = line.split(" => ", 1)
        source = Path(path.strip())
        if source.is_absolute() and source.is_file():
            libraries.setdefault(name.strip().split()[0], source)
    missing = set(DLOPEN_LIBRARIES) - libraries.keys()
    if missing:
        raise ValueError("Missing dynamically loaded desktop libraries: " + ", ".join(sorted(missing)))
    return libraries


def library_package(source):
    # usr-merged systems may index either spelling of the same installed file.
    # Retry only these proven aliases; other tool/resource failures propagate.
    aliases = [source, source.resolve()]
    resolved = source.resolve()
    if resolved.is_relative_to("/usr/lib"):
        aliases.append(Path("/") / resolved.relative_to("/usr"))
    for alias in dict.fromkeys(aliases):
        if not alias.is_file() or alias.resolve() != resolved:
            continue
        try:
            ownership = run(["dpkg-query", "--search", alias]).strip().splitlines()
        except ToolFailure:
            continue
        if len(ownership) != 1 or ": " not in ownership[0]:
            raise ValueError("Cannot establish bundled library package ownership")
        package = ownership[0].split(": ", 1)[0]
        if not PACKAGE_NAME.fullmatch(package):
            raise ValueError("Ambiguous bundled library package ownership")
        return package
    raise ValueError("Cannot identify the installed package owning a bundled library")


def native_library_notices(appdir, libraries):
    deployed = {}
    for path in (appdir / "usr/lib").rglob("*"):
        if path.is_file() and ".so" in path.name:
            resolved = path.resolve()
            if not resolved.is_relative_to(appdir.resolve()):
                raise ValueError("Bundled library link escapes AppDir")
            previous = deployed.get(resolved.name)
            if previous is not None and contract.digest(previous) != contract.digest(resolved):
                raise ValueError("Bundled libraries with the same name have different bytes")
            deployed.setdefault(resolved.name, resolved)
    sources = {}
    for source in libraries.values():
        sources.setdefault(source.name, source)
        sources.setdefault(source.resolve().name, source)
    sections = []
    records = []
    packages = set()
    for name in sorted(deployed):
        source = sources.get(name)
        if source is None:
            raise ValueError("Cannot identify the installed source of a bundled library")
        package = library_package(source)
        packages.add(package)
        records.append({
            "name": name,
            "sha256": contract.digest(deployed[name]),
            "source_name": source.resolve().name,
            "source_sha256": contract.digest(source.resolve()),
            "package": package,
        })
    common = set()
    versions = {}
    for package in sorted(packages):
        identity = run(["dpkg-query", "--show", "--showformat=${binary:Package} ${Version}\\n", package]).strip()
        license_path = Path("/usr/share/doc") / package.split(":", 1)[0] / "copyright"
        contract.regular(license_path.resolve(), contract.MAX_LEGAL_BYTES)
        text = license_path.read_text("utf-8")
        versions[package] = identity
        sections.append(f"Package {identity}\n{text}\n")
        common.update(COMMON_LICENSE.findall(text))
    for name in sorted(common):
        text = contract.regular(Path("/usr/share/common-licenses") / name, contract.MAX_LEGAL_BYTES).read_text("utf-8")
        sections.append(f"Common license {name}\n{text}\n")
    return "\n".join(sections), {"libraries": records, "packages": versions}


def validate_runtime_source(path):
    contract.regular(path, installers.MAX_INSTALLER_BYTES)
    with path.open("rb") as stream:
        if stream.read(2) != b"\x1f\x8b":
            raise ValueError("AppImage runtime source must be a gzip-compressed source kit")
    count = 0
    total = 0
    with tarfile.open(path, "r:gz") as archive:
        for member in archive:
            count += 1
            total += member.size
            if count > MAX_RUNTIME_SOURCE_MEMBERS or total > installers.MAX_INSTALLER_BYTES:
                raise ValueError("AppImage runtime source kit exceeds its bounds")
            parts = member.name.split("/")
            if member.name.startswith("/") or ".." in parts or "\\" in member.name:
                raise ValueError("AppImage runtime source kit has unsafe member paths")
            if not member.isfile() and not member.isdir():
                raise ValueError("AppImage runtime source kit must contain regular files and directories")
    if count == 0:
        raise ValueError("AppImage runtime source kit is empty")


def validate_appimage_tools(inputs, tools):
    if tools is None:
        raise ValueError("AppImage requires tools, a reviewed runtime, notices and corresponding source")
    for tool in (tools.linuxdeploy, tools.appimagetool, tools.runtime):
        contract.regular(tool)
    validate_runtime_source(tools.runtime_source)
    contract.regular(tools.runtime_notices, contract.MAX_LEGAL_BYTES)
    if not tools.runtime_notices.read_text("utf-8").strip():
        raise ValueError("Reviewed AppImage runtime notices are required")
    with tools.runtime.open("rb") as stream:
        contract.architecture(stream, tools.runtime.stat().st_size, inputs.target)


def build_appimage(inputs, root, output, temporary, tools):
    libraries = library_sources(inputs.target)
    command = [tools.linuxdeploy.resolve(), "--appdir", root.resolve(), "--executable", (root / APPIMAGE_PAYLOAD / BINARY).resolve(), "--desktop-file", (root / f"usr/share/applications/{APPLICATION_ID}.desktop").resolve(), "--icon-file", (root / f"usr/share/icons/hicolor/{ICON_SIZE}x{ICON_SIZE}/apps/{APPLICATION_ID}.png").resolve()]
    for name in DLOPEN_LIBRARIES:
        command.extend(("--library", libraries[name]))
    environment = os.environ.copy()
    environment["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    environment["ARCH"] = TARGETS[inputs.target][0]
    environment["VERSION"] = inputs.version
    run(command, environment=environment)
    # linuxdeploy owns dependency and RPATH rewriting. Hash the actual resulting
    # executable only after it has completed, never the pre-patched input.
    release_files(root / APPIMAGE_PAYLOAD, inputs)
    app_run = root / "AppRun"
    if app_run.exists() or app_run.is_symlink():
        app_run.unlink()
    copy(ASSETS / "AppRun", app_run, 0o755)
    runtime_notices = tools.runtime_notices.read_text("utf-8")
    native_notices, provenance = native_library_notices(root, libraries)
    provenance["runtime_sha256"] = contract.digest(tools.runtime)
    provenance["runtime_notices_sha256"] = contract.digest(tools.runtime_notices)
    provenance["runtime_source_sha256"] = contract.digest(tools.runtime_source)
    copy(tools.runtime_source, root / LICENSE_DIRECTORY / "RuntimeSource.tar.gz")
    provenance["linuxdeploy_sha256"] = contract.digest(tools.linuxdeploy)
    provenance["appimagetool_sha256"] = contract.digest(tools.appimagetool)
    write(root / LICENSE_DIRECTORY / "LinuxRuntimeSources.json", (json.dumps(provenance, indent=2, sort_keys=True) + "\n").encode("utf-8"))
    text = "AppImage runtime notices\n\n" + runtime_notices + "\n\nBundled system libraries\n\n" + native_notices
    if len(text.encode("utf-8")) > contract.MAX_LEGAL_BYTES:
        raise ValueError("Bundled native notices exceed the legal-payload bound")
    write(root / LICENSE_DIRECTORY / "LinuxRuntimeLicenses.txt", text.encode("utf-8"))
    run([tools.appimagetool.resolve(), "--runtime-file", tools.runtime.resolve(), root.resolve(), output.resolve()], environment=environment)
    output.chmod(0o755)
    verify_appimage(output, root, inputs, temporary)


def package(inputs, output, formats, tools=None):
    validate_inputs(inputs)
    formats = tuple(formats)
    if not formats or len(set(formats)) != len(formats) or any(kind not in installers.formats(inputs.target) for kind in formats):
        raise ValueError("Choose distinct supported Linux formats")
    if "appimage" in formats:
        validate_appimage_tools(inputs, tools)
    if output.is_symlink():
        raise ValueError("Output directory cannot be a symlink")
    output.mkdir(parents=True, exist_ok=True)
    for kind in formats:
        destination = output / installers.name(inputs.version, inputs.target, kind)
        sidecar = destination.with_name(destination.name + ".sha256")
        if destination.exists() or destination.is_symlink() or sidecar.exists() or sidecar.is_symlink():
            raise ValueError("Linux package and checksum paths must be fresh")
    for kind in formats:
        destination = output / installers.name(inputs.version, inputs.target, kind)
        created = []
        try:
            with tempfile.TemporaryDirectory(prefix="tablecraft-linux-package-") as directory:
                temporary = Path(directory)
                root = temporary / "AppDir" if kind == "appimage" else temporary / "root"
                root.mkdir()
                stage(root, inputs, appimage=kind == "appimage")
                built = temporary / destination.name
                if kind == "deb":
                    build_deb(inputs, root, built, temporary)
                elif kind == "rpm":
                    build_rpm(inputs, root, built, temporary)
                else:
                    build_appimage(inputs, root, built, temporary, tools)
                installers.verify_container(built, inputs.version, inputs.target, kind)
                with built.open("rb") as source:
                    with destination.open("xb") as target:
                        created.append(destination)
                        shutil.copyfileobj(source, target, desktop.COPY_BYTES)
                if kind == "appimage":
                    destination.chmod(0o755)
                sidecar = destination.with_name(destination.name + ".sha256")
                with sidecar.open("xb") as stream:
                    created.append(sidecar)
                    stream.write(installers.checksum(destination))
                print(destination)
        except BaseException:
            for path in reversed(created):
                path.unlink(missing_ok=True)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--icon", type=Path, required=True, help="Generated 256-pixel application PNG")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--formats", nargs="+", choices=("deb", "rpm", "appimage"), default=("deb", "rpm", "appimage"))
    parser.add_argument("--linuxdeploy", type=Path)
    parser.add_argument("--appimagetool", type=Path)
    parser.add_argument("--appimage-runtime", type=Path)
    parser.add_argument("--appimage-runtime-notices", type=Path)
    parser.add_argument("--appimage-runtime-source", type=Path)
    args = parser.parse_args()
    tools = None
    values = (args.linuxdeploy, args.appimagetool, args.appimage_runtime, args.appimage_runtime_notices, args.appimage_runtime_source)
    if any(value is not None for value in values):
        if not all(value is not None for value in values):
            parser.error("Supply all five AppImage tool/runtime inputs")
        tools = AppImageTools(*values)
    try:
        package(Inputs(args.version, args.target, args.binary, args.notices, args.icon), args.output, args.formats, tools)
    except (OSError, ValueError, subprocess.SubprocessError, tarfile.TarError) as error:
        parser.exit(1, f"Linux packaging failed: {error}\n")


if __name__ == "__main__":
    main()
