"""Native installer identities and platform-independent assembly framing checks.

Native build jobs perform full tool-based payload/metadata verification. Assembly
checks their exact filenames, container framing and bytes; it never executes a
foreign installer or claims to repeat native installation verification.
"""
import release_contract as contract

MAX_INSTALLER_BYTES = 1024 * 1024 * 1024
CFB_HEADER = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"
DEB_HEADER = b"!<arch>\n"
RPM_HEADER = b"\xed\xab\xee\xdb"
DMG_TRAILER_BYTES = 512


def formats(target):
    if target not in contract.TARGETS:
        raise ValueError("Unsupported installer target")
    if "windows" in target:
        return ("msi",)
    if "apple" in target:
        return ("dmg",)
    return ("deb", "rpm", "appimage")


def name(version, target, kind):
    contract.validate_version(version)
    if kind not in formats(target):
        raise ValueError("Installer format does not match target")
    extension = "AppImage" if kind == "appimage" else kind
    return f"tablecraft-desktop-{version}-{contract.TARGETS[target]}.{extension}"


def names(version):
    return {name(version, target, kind)
            for target in contract.TARGETS for kind in formats(target)}


def verify_container(path, version, target, kind):
    if path.name != name(version, target, kind):
        raise ValueError("Installer filename does not match its release identity")
    contract.regular(path, MAX_INSTALLER_BYTES)
    size = path.stat().st_size
    with path.open("rb") as stream:
        header = stream.read(64)
        if kind == "msi":
            valid = size >= 512 and size % 512 == 0 and header.startswith(CFB_HEADER)
        elif kind == "dmg":
            valid = size >= DMG_TRAILER_BYTES
            if valid:
                stream.seek(-DMG_TRAILER_BYTES, 2)
                valid = stream.read(4) == b"koly"
        elif kind == "deb":
            valid = header.startswith(DEB_HEADER)
        elif kind == "rpm":
            valid = size >= 96 and header.startswith(RPM_HEADER)
        elif kind == "appimage":
            stream.seek(0)
            contract.architecture(stream, size, target)
            valid = header[8:11] == b"AI\x02"
        else:
            valid = False
        if not valid:
            raise ValueError("Installer container framing does not match its format")


def checksum(path):
    return f"{contract.digest(path)}  {path.name}\n".encode("ascii")


def write_checksum(path):
    contract.regular(path, MAX_INSTALLER_BYTES)
    sidecar = path.with_name(path.name + ".sha256")
    with sidecar.open("xb") as stream:
        stream.write(checksum(path))
    return sidecar
