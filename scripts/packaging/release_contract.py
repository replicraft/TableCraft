"""Version, native executable and legal-payload contracts for CLI archives."""
import hashlib
import json
from pathlib import Path
import re
import struct
import tomllib

ROOT = Path(__file__).resolve().parents[2]
TARGETS = {
    "x86_64-pc-windows-msvc": "windows-x64",
    "aarch64-pc-windows-msvc": "windows-arm64",
    "x86_64-apple-darwin": "macos-x64",
    "aarch64-apple-darwin": "macos-arm64",
    "x86_64-unknown-linux-gnu": "linux-x64",
    "aarch64-unknown-linux-gnu": "linux-arm64",
}
VERSION = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?\Z"
)
EXECUTABLE_HEADER_BYTES = 64
PE_DOS_SIGNATURE = b"MZ"
PE_SIGNATURE = b"PE\0\0"
PE_POINTER_OFFSET = 60
PE_HEADER_BYTES = 24
PE_MACHINE_PREFIX_BYTES = 6
PE_FLAGS_OFFSET = 22
PE_EXECUTABLE_FLAG = 0x0002
PE_DLL_FLAG = 0x2000
PE_MAX_HEADER_OFFSET = 16 * 1024 * 1024
PE_ARM64_MACHINE = 0xaa64
PE_X64_MACHINE = 0x8664
MACH_MAGIC_64 = 0xfeedfacf
MACH_ARM64_CPU = 0x0100000c
MACH_X64_CPU = 0x01000007
MACH_MAGIC_CPU_BYTES = 8
MACH_FILE_TYPE_OFFSET = 12
MACH_EXECUTE = 2
ELF_IDENT = b"\x7fELF\x02\x01\x01"
ELF_ABI_OFFSET = 7
ELF_SUPPORTED_ABIS = (0, 3)
ELF_TYPE_OFFSET = 16
ELF_MACHINE_OFFSET = 18
ELF_EXECUTABLE_TYPES = (2, 3)
ELF_ARM64_MACHINE = 183
ELF_X64_MACHINE = 62
MAX_EXECUTABLE_BYTES = 512 * 1024 * 1024
MAX_LEGAL_BYTES = 16 * 1024 * 1024
MACOS_LAUNCHER = "TableCraft.app/Contents/MacOS/TableCraft"
MACOS_CLI = "TableCraft.app/Contents/Resources/tablecraft"
MACOS_INFO = "TableCraft.app/Contents/Info.plist"


def macos_info(version):
    types = [
        ("Microsoft Access MDB database", "mdb"),
        ("Microsoft Access ACCDB database", "accdb"),
    ]
    return {
        "CFBundleIdentifier": "org.replicraft.tablecraft",
        "CFBundleName": "TableCraft",
        "CFBundleExecutable": "TableCraft",
        "CFBundlePackageType": "APPL",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleShortVersionString": version.split("-", 1)[0],
        "CFBundleVersion": version.split("-", 1)[0],
        "LSMinimumSystemVersion": "13.0",
        "LSUIElement": True,
        "CFBundleDocumentTypes": [
            {
                "CFBundleTypeName": name,
                "CFBundleTypeRole": "Viewer",
                "LSHandlerRank": "Alternate",
                # Use extension matching until the formats' canonical foreign
                # UTIs are established. Invented imported UTIs compete with the
                # identifiers supplied by existing database applications.
                "CFBundleTypeExtensions": [extension],
            }
            for name, extension in types
        ],
    }


def handler_names(target):
    if "windows" in target:
        return {"register-file-associations.ps1"}
    if "apple" in target:
        return {MACOS_LAUNCHER, MACOS_CLI, MACOS_INFO}
    return {"file_associations.py"}


def static_handler_payload(target):
    if "apple" in target:
        return {}
    platform = "windows" if "windows" in target else "linux"
    return {
        name: regular(ROOT / "packaging" / platform / name, MAX_LEGAL_BYTES)
        for name in handler_names(target)
    }


def executable_member(name, target):
    return name in {executable_name(target), MACOS_LAUNCHER, MACOS_CLI}


def regular(path, maximum=None):
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise ValueError(f"Expected a nonempty regular file: {path}")
    if maximum is not None and path.stat().st_size > maximum:
        raise ValueError(f"File exceeds release limit: {path}")
    return path


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def workspace_version():
    return tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


def validate_version(version, tag=None):
    if not VERSION.fullmatch(version) or version != workspace_version():
        raise ValueError("Release version must be safe SemVer matching workspace.package.version")
    if "-" in version:
        identifiers = version.split("-", 1)[1].split(".")
        if any(value.isdigit() and len(value) > 1 and value.startswith("0") for value in identifiers):
            raise ValueError("Numeric prerelease identifiers cannot contain leading zeroes")
    if tag is not None and tag != "v" + version:
        raise ValueError("Release tag must equal v<workspace version>")
    notes = regular(ROOT / "docs" / "releases" / f"v{version}.md")
    if not notes.read_text(encoding="utf-8").strip():
        raise ValueError("Release notes must not be blank")


def executable_name(target):
    return "tablecraft.exe" if "windows" in target else "tablecraft"


def archive_name(version, target):
    extension = ".zip" if "windows" in target else ".tar.gz"
    return f"tablecraft-cli-{version}-{TARGETS[target]}{extension}"


def architecture(stream, size, target):
    if target not in TARGETS or size < EXECUTABLE_HEADER_BYTES or size > MAX_EXECUTABLE_BYTES:
        raise ValueError("Unsupported target or executable size")
    stream.seek(0)
    header = stream.read(EXECUTABLE_HEADER_BYTES)
    if len(header) != EXECUTABLE_HEADER_BYTES:
        raise ValueError("Truncated executable header")
    arm = target.startswith("aarch64")
    if "windows" in target:
        valid = header[:len(PE_DOS_SIGNATURE)] == PE_DOS_SIGNATURE
        offset = struct.unpack_from("<I", header, PE_POINTER_OFFSET)[0]
        if valid and EXECUTABLE_HEADER_BYTES <= offset <= min(size - PE_HEADER_BYTES, PE_MAX_HEADER_OFFSET):
            stream.seek(offset)
            pe = stream.read(PE_HEADER_BYTES)
            machine = PE_ARM64_MACHINE if arm else PE_X64_MACHINE
            flags = struct.unpack_from("<H", pe, PE_FLAGS_OFFSET)[0] if len(pe) == PE_HEADER_BYTES else 0
            valid = pe[:PE_MACHINE_PREFIX_BYTES] == PE_SIGNATURE + struct.pack("<H", machine)
            valid = valid and bool(flags & PE_EXECUTABLE_FLAG) and not bool(flags & PE_DLL_FLAG)
        else:
            valid = False
    elif "apple" in target:
        cpu = MACH_ARM64_CPU if arm else MACH_X64_CPU
        valid = header[:MACH_MAGIC_CPU_BYTES] == struct.pack("<II", MACH_MAGIC_64, cpu)
        valid = valid and struct.unpack_from("<I", header, MACH_FILE_TYPE_OFFSET)[0] == MACH_EXECUTE
    else:
        machine = ELF_ARM64_MACHINE if arm else ELF_X64_MACHINE
        valid = header[:len(ELF_IDENT)] == ELF_IDENT and header[ELF_ABI_OFFSET] in ELF_SUPPORTED_ABIS
        valid = valid and struct.unpack_from("<H", header, ELF_TYPE_OFFSET)[0] in ELF_EXECUTABLE_TYPES
        valid = valid and struct.unpack_from("<H", header, ELF_MACHINE_OFFSET)[0] == machine
    if not valid:
        raise ValueError(f"Executable header does not match {target}")


def legal_payload():
    return {
        "LICENSE": regular(ROOT / "LICENSE", MAX_LEGAL_BYTES),
        "ThirdPartyLicenses.txt": regular(
            ROOT / "packaging" / "ThirdPartyLicenses.txt", MAX_LEGAL_BYTES,
        ),
    }


def manifest(version, target, files):
    return (json.dumps({
        "product": "tablecraft-cli", "version": version, "target": target,
        "files": {name: digest(path) for name, path in sorted(files.items())},
    }, indent=2) + "\n").encode("utf-8")
