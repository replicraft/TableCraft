"""Pure association metadata for a future desktop host, never an installer.

Callers must supply the eventual GUI artifact explicitly. Rendering does not
establish that it exists or implements the file-receive contract. The future
host must receive one literal local filename after "--", without evaluating
stored database content. No current release consumes these payloads.
"""
from dataclasses import dataclass
from pathlib import PurePosixPath, PureWindowsPath


APPLICATION_ID = "org.replicraft.tablecraft"
MIME_TYPE = "application/vnd.ms-access"
EXTENSIONS = ("mdb", "accdb")
REGISTRY_ROOT = "HKEY_CURRENT_USER\\Software"
CAPABILITIES = "Software\\TableCraft\\Capabilities"


@dataclass(frozen=True)
class RegistryValue:
    key: str
    name: str
    kind: str
    value: str | bytes


def _text(value):
    if not isinstance(value, (str, PurePosixPath, PureWindowsPath)):
        raise ValueError("An explicit GUI executable path is required")
    result = str(value)
    if not result or any(ord(character) < 32 or ord(character) == 127 for character in result):
        raise ValueError("Executable paths must not contain control characters")
    return result


def windows_records(gui_executable):
    """Describe per-user Open With values without choosing a default handler.

    Percent signs are refused because the command contains a ShellExecute file
    substitution. These REG_SZ values are not environment-expanded commands.
    """
    value = _text(gui_executable)
    executable = PureWindowsPath(value)
    if not executable.is_absolute() or executable.suffix.lower() != ".exe":
        raise ValueError("An absolute Windows GUI .exe path is required")
    if "%" in value or any(character in '<>"|?*' for character in value):
        raise ValueError("Unsupported Windows executable path characters")
    if any(":" in part for part in executable.parts[1:]) or ".." in executable.parts:
        raise ValueError("Unsupported Windows executable path components")

    command = f'"{executable}" -- "%1"'
    records = []
    for extension in EXTENSIONS:
        identifier = "TableCraft.Database." + extension
        key = REGISTRY_ROOT + "\\Classes\\" + identifier
        records.extend((
            RegistryValue(key, "", "REG_SZ", "Microsoft Access " + extension.upper() + " database"),
            RegistryValue(key + "\\shell\\open\\command", "", "REG_SZ", command),
            RegistryValue(
                REGISTRY_ROOT + "\\Classes\\." + extension + "\\OpenWithProgids",
                identifier,
                "REG_NONE",
                b"",
            ),
            RegistryValue(
                "HKEY_CURRENT_USER\\" + CAPABILITIES + "\\FileAssociations",
                "." + extension,
                "REG_SZ",
                identifier,
            ),
        ))

    records.extend((
        RegistryValue("HKEY_CURRENT_USER\\" + CAPABILITIES, "ApplicationName", "REG_SZ", "TableCraft"),
        RegistryValue(
            "HKEY_CURRENT_USER\\" + CAPABILITIES,
            "ApplicationDescription",
            "REG_SZ",
            "Desktop viewer for Microsoft Access databases",
        ),
        RegistryValue(
            REGISTRY_ROOT + "\\RegisteredApplications",
            "TableCraft",
            "REG_SZ",
            CAPABILITIES,
        ),
    ))
    return tuple(records)


def _desktop_executable(gui_executable):
    value = _text(gui_executable)
    executable = PurePosixPath(value)
    if not executable.is_absolute() or not executable.name or value.endswith("/"):
        raise ValueError("An absolute Linux GUI executable path is required")
    if "=" in value or ".." in executable.parts:
        raise ValueError("Unsupported desktop executable path components")

    # Exec quoting and desktop-entry string escaping are separate layers.
    escaped = "".join(
        "\\" + character if character in ("\\", '"', "$", "`") else character
        for character in value
    )
    return '"' + escaped.replace("\\", "\\\\").replace("%", "%%") + '"'


def linux_payloads(gui_executable):
    """Return desktop/MIME files without writing files or changing defaults."""
    executable = _desktop_executable(gui_executable)
    desktop = (
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=TableCraft\n"
        f"Exec={executable} -- %f\n"
        "Terminal=false\n"
        f"MimeType={MIME_TYPE};\n"
        "Categories=Office;Database;\n"
    ).encode("utf-8")
    mime = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">\n'
        f'  <mime-type type="{MIME_TYPE}">\n'
        "    <comment>Microsoft Access database</comment>\n"
        '    <glob pattern="*.mdb"/>\n'
        '    <glob pattern="*.accdb"/>\n'
        "  </mime-type>\n"
        "</mime-info>\n"
    ).encode("utf-8")
    return {
        "applications/" + APPLICATION_ID + ".desktop": desktop,
        "mime/packages/" + APPLICATION_ID + ".xml": mime,
    }


def macos_document_types():
    """Render only the future app's document fragment, not a runnable bundle.

    Extension-only matching avoids claiming invented identifiers for foreign
    formats. This is Apple's deprecated extension contract, not a replacement
    for qualifying native Open With behavior with other installed viewers.
    """
    return {
        "CFBundleDocumentTypes": [
            {
                "CFBundleTypeName": "Microsoft Access " + extension.upper() + " database",
                "CFBundleTypeExtensions": [extension],
                "CFBundleTypeRole": "Viewer",
                "LSHandlerRank": "Alternate",
            }
            for extension in EXTENSIONS
        ],
    }
