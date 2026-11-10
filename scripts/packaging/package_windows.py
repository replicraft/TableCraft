#!/usr/bin/env python3
"""Build and inspect real native x64/ARM64 desktop MSI installers with WiX."""
import argparse
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

import file_associations
import installer_contract
import package_desktop as desktop
import release_contract as contract

ARCHITECTURES = {"x86_64-pc-windows-msvc": "x64", "aarch64-pc-windows-msvc": "arm64"}
# Stable first-party identity derived once from TableCraft's installer namespace.
UPGRADE_CODE = "f78632a2-14b8-5f28-b29d-760bded29a9a"
WIX_NAMESPACE = "http://wixtoolset.org/schemas/v4/wxs"
TEMPLATE = contract.ROOT / "packaging/windows/tablecraft.wxs"
INSPECTOR = contract.ROOT / "packaging/windows/inspect_msi.ps1"
MAX_ICON_BYTES = 16 * 1024 * 1024
MAX_ICON_IMAGES = 256
ICO_HEADER_BYTES = 6
ICO_ENTRY_BYTES = 16
MAX_MSI_BYTES = 768 * 1024 * 1024
MSI_64BIT_COMPONENT = 256
MAX_INSPECTION_BYTES = 1024 * 1024
REGISTRATION_COMPONENT = "ApplicationRegistration"
COMMAND_PLACEHOLDER = r"C:\TableCraft\tablecraft-desktop.exe"


def msi_version(version):
    contract.validate_version(version)
    if "-" in version or "+" in version:
        raise ValueError("MSI packaging requires a stable three-component release version")
    values = tuple(int(part) for part in version.split("."))
    if len(values) != 3 or any(value > limit for value, limit in zip(values, (255, 255, 65535))):
        raise ValueError("Release version exceeds Windows Installer version limits")
    return version


def validate_icon(path):
    data = contract.regular(path, MAX_ICON_BYTES).read_bytes()
    if len(data) < ICO_HEADER_BYTES:
        raise ValueError("Truncated ICO header")
    reserved, kind, count = struct.unpack_from("<HHH", data)
    end = ICO_HEADER_BYTES + count * ICO_ENTRY_BYTES
    if reserved != 0 or kind != 1 or not 1 <= count <= MAX_ICON_IMAGES or end > len(data):
        raise ValueError("Invalid ICO image directory")
    ranges = []
    for offset in range(ICO_HEADER_BYTES, end, ICO_ENTRY_BYTES):
        width, height, _, reserved, planes, bits, size, start = struct.unpack_from("<BBBBHHII", data, offset)
        width, height = width or 256, height or 256
        if reserved != 0 or width != height or planes not in (0, 1) or bits not in (0, 32):
            raise ValueError("Unsupported ICO image metadata")
        if size == 0 or start < end or start + size > len(data):
            raise ValueError("ICO image lies outside its file")
        desktop.validate_png(data[start:start + size], width)
        ranges.append((start, start + size))
    ranges.sort()
    if any(left[1] > right[0] for left, right in zip(ranges, ranges[1:])):
        raise ValueError("Overlapping ICO image records")


def registry_values():
    """Reuse existing ProgIDs/capabilities while installing them machine-wide."""
    values = []
    for index, record in enumerate(file_associations.windows_records(COMMAND_PLACEHOLDER)):
        prefix = "HKEY_CURRENT_USER\\"
        if not record.key.startswith(prefix) or record.kind not in ("REG_SZ", "REG_NONE"):
            raise ValueError("Unsupported application registration record")
        value = "" if record.kind == "REG_NONE" else record.value
        value = value.replace(COMMAND_PLACEHOLDER, "[#TableCraftDesktop]")
        values.append({
            "id": f"RegistrationValue{index:02}",
            "root": "2",  # Windows Installer Registry table: HKEY_LOCAL_MACHINE.
            "key": record.key[len(prefix):],
            "name": record.name,
            "value": value,
            "component": REGISTRATION_COMPONENT,
        })
    return values


def write_registry(path):
    ET.register_namespace("", WIX_NAMESPACE)
    include = ET.Element(f"{{{WIX_NAMESPACE}}}Include")
    for index, value in enumerate(registry_values()):
        attributes = {"Id": value["id"], "Root": "HKLM", "Key": value["key"],
                      "Type": "string", "Value": value["value"]}
        if value["name"]:
            attributes["Name"] = value["name"]
        if index == 0:
            attributes["KeyPath"] = "yes"
        ET.SubElement(include, f"{{{WIX_NAMESPACE}}}RegistryValue", attributes)
    ET.indent(include, space="  ")
    ET.ElementTree(include).write(path, encoding="utf-8", xml_declaration=True)


def preflight(version, target, binary, notices, icon):
    msi_version(version)
    installer_contract.name(version, target, "msi")
    contract.regular(binary, contract.MAX_EXECUTABLE_BYTES)
    with binary.open("rb") as source:
        contract.architecture(source, binary.stat().st_size, target)
    for path in (contract.ROOT / "LICENSE", notices):
        if not contract.regular(path, contract.MAX_LEGAL_BYTES).read_text("utf-8").strip():
            raise ValueError("Installer legal payload must not be blank")
    validate_icon(icon)


def stage_payload(version, target, binary, notices, stage):
    stage.mkdir()
    for name, source in (("tablecraft-desktop.exe", binary), ("LICENSE", contract.ROOT / "LICENSE"),
                         ("ThirdPartyLicenses.txt", notices)):
        shutil.copyfile(source, stage / name)
    files = {name: stage / name for name in ("tablecraft-desktop.exe", "LICENSE", "ThirdPartyLicenses.txt")}
    metadata = json.loads(desktop.manifest(version, target, files))
    # Windows Installer owns these files even when INSTALLFOLDER is customized.
    # Keep this identity distinct from a replaceable portable archive.
    metadata["installation"] = "windows-msi"
    (stage / "RELEASE.json").write_text(
        json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )


def build_command(version, target, stage, icon, registration, output, tool):
    return [tool, "build", "-arch", ARCHITECTURES[target], "-pdbtype", "none",
            "-d", f"Version={version}", "-d", f"UpgradeCode={UPGRADE_CODE}",
            "-d", f"Payload={stage}", "-d", f"Icon={icon}",
            "-d", f"RegistryInclude={registration}", "-o", str(output), str(TEMPLATE)]


def inspect_tables(installer, powershell="powershell.exe"):
    result = subprocess.run([powershell, "-NoLogo", "-NoProfile", "-NonInteractive",
                             "-ExecutionPolicy", "Bypass", "-File", str(INSPECTOR),
                             "-Path", str(installer)], check=True, capture_output=True)
    if len(result.stdout) > MAX_INSPECTION_BYTES:
        raise ValueError("MSI table report exceeds inspection limit")
    evidence = os.environ.get("TABLECRAFT_MSI_EVIDENCE_DIRECTORY")
    if evidence:
        directory = Path(evidence)
        directory.mkdir(parents=True, exist_ok=False)
        with (directory / "inspection.json").open("xb") as stream:
            stream.write(result.stdout)
    return json.loads(result.stdout.decode("utf-8-sig"))


def validate_report_shape(report):
    columns = {
        "properties": {"name", "value"},
        "files": {"id", "component", "name", "size"},
        "directories": {"id", "parent", "name"},
        "components": {"id", "directory", "attributes", "keypath"},
        "shortcuts": {"id", "directory", "name", "component", "target", "icon"},
        "registry": {"id", "root", "key", "name", "value", "component"},
        "upgrades": {"code", "minimum", "maximum", "attributes", "action"},
        "icons": {"name"},
        "customActions": {"action", "type", "source", "target"},
    }
    if not isinstance(report, dict) or set(report) != {"template", *columns}:
        raise ValueError("MSI inspection report has unexpected tables")
    if not isinstance(report["template"], str):
        raise ValueError("MSI inspection summary is not a string")
    for table, expected in columns.items():
        rows = report[table]
        if not isinstance(rows, list) or len(rows) > 256:
            raise ValueError(f"MSI inspection {table} is not a bounded row array")
        for index, row in enumerate(rows):
            if not isinstance(row, dict) or set(row) != expected:
                raise ValueError(f"MSI inspection {table} row {index} has unexpected columns")
            if any(not isinstance(value, str) for value in row.values()):
                raise ValueError(f"MSI inspection {table} row {index} has a non-string field")


def check_tables(report, version, target, payload):
    validate_report_shape(report)
    if report["customActions"]:
        raise ValueError("MSI must not contain application-launch or other custom actions")
    properties = {row["name"]: row["value"] for row in report["properties"]}
    expected = {"ProductName": "TableCraft", "Manufacturer": "Replicraft",
                "ProductVersion": version, "ALLUSERS": "1", "ARPPRODUCTICON": "TableCraftIcon"}
    if any(properties.get(key) != value for key, value in expected.items()):
        raise ValueError("MSI product identity differs from the release")
    if properties.get("UpgradeCode", "").strip("{}").lower() != UPGRADE_CODE:
        raise ValueError("MSI upgrade identity differs")
    if report["template"].split(";", 1)[0].lower() != ARCHITECTURES[target].lower():
        raise ValueError("MSI template architecture differs")
    expected_files = {path.name: str(path.stat().st_size) for path in payload.iterdir()}
    actual_files = {row["name"].split("|")[-1]: row["size"] for row in report["files"]}
    if actual_files != expected_files or len(report["files"]) != len(expected_files):
        raise ValueError("MSI file payload differs")
    file_ids = {row["id"]: row["name"].split("|")[-1] for row in report["files"]}
    if file_ids != {"TableCraftDesktop": "tablecraft-desktop.exe", "ProjectLicenseFile": "LICENSE",
                    "ThirdPartyNoticesFile": "ThirdPartyLicenses.txt", "ReleaseManifestFile": "RELEASE.json"}:
        raise ValueError("MSI file identities differ from application registrations")
    components = {row["id"]: row for row in report["components"]}
    expected_keys = {"DesktopApplication": "TableCraftDesktop", "ProjectLicense": "ProjectLicenseFile",
                     "ThirdPartyNotices": "ThirdPartyNoticesFile", "ReleaseManifest": "ReleaseManifestFile",
                     REGISTRATION_COMPONENT: "RegistrationValue00"}
    if len(report["components"]) != len(expected_keys) or set(components) != set(expected_keys):
        raise ValueError("MSI component ownership set differs")
    for name, keypath in expected_keys.items():
        row = components[name]
        expected_attributes = MSI_64BIT_COMPONENT | (4 if name == REGISTRATION_COMPONENT else 0)
        if row["directory"] != "INSTALLFOLDER" or row["keypath"] != keypath or int(row["attributes"]) != expected_attributes:
            raise ValueError("MSI component architecture, lifetime or keypath differs")
    directories = {row["id"]: row for row in report["directories"]}
    installed = directories.get("INSTALLFOLDER", {})
    if installed.get("parent") not in ("ProgramFiles64Folder", "ProgramFiles6432Folder") or installed.get("name", "").split("|")[-1] != "TableCraft":
        raise ValueError("MSI installation directory differs")
    shortcuts = report["shortcuts"]
    if len(shortcuts) == 1:
        shortcuts = [{**shortcuts[0], "name": shortcuts[0]["name"].split("|")[-1]}]
    if len(shortcuts) != 1 or shortcuts[0] != {"id": "TableCraftStartMenu", "directory": "ProgramMenuFolder",
            "name": "TableCraft", "component": "DesktopApplication", "target": "DesktopFeature", "icon": "TableCraftIcon"}:
        raise ValueError("MSI Start Menu shortcut is missing or unexpected")
    actual_registry = [{key: row[key] for key in ("id", "root", "key", "name", "value", "component")} for row in report["registry"]]
    if sorted(actual_registry, key=lambda row: row["id"]) != registry_values():
        raise ValueError("MSI application registrations differ from the shared handler contract")
    if {row["action"] for row in report["upgrades"]} != {"WIX_UPGRADE_DETECTED", "WIX_DOWNGRADE_DETECTED"} or any(row["code"].strip("{}").lower() != UPGRADE_CODE for row in report["upgrades"]):
        raise ValueError("MSI major-upgrade ownership is missing")
    if report["icons"] != [{"name": "TableCraftIcon"}]:
        raise ValueError("MSI application icon resource is missing")


def verify(installer, version, target, payload, powershell="powershell.exe", msiexec="msiexec.exe"):
    """Inspect MSI tables and administratively extract payloads without installing."""
    contract.regular(installer, MAX_MSI_BYTES)
    installer_contract.verify_container(installer, version, target, "msi")
    check_tables(inspect_tables(installer, powershell), version, target, payload)
    with tempfile.TemporaryDirectory(prefix="tablecraft-msi-extract-") as temporary:
        destination = Path(temporary) / "administrative-image"
        subprocess.run([msiexec, "/a", str(installer.resolve()), "/qn", "/norestart",
                        f"TARGETDIR={destination}"], check=True, capture_output=True)
        for expected in payload.iterdir():
            found = [path for path in destination.rglob(expected.name) if path.is_file() and not path.is_symlink()]
            if len(found) != 1 or contract.digest(found[0]) != contract.digest(expected):
                raise ValueError(f"Extracted MSI payload differs: {expected.name}")


def package(version, target, binary, notices, icon, output, tool="wix"):
    preflight(version, target, binary, notices, icon)
    if sys.platform != "win32":
        raise ValueError("Real MSI creation and table inspection require native Windows")
    if output.is_symlink():
        raise ValueError("Installer output directory must not be a symlink")
    output.mkdir(parents=True, exist_ok=True)
    installer = output / installer_contract.name(version, target, "msi")
    checksum = installer.with_name(installer.name + ".sha256")
    if installer.exists() or installer.is_symlink() or checksum.exists() or checksum.is_symlink():
        raise ValueError("Installer and checksum outputs must be fresh")
    with tempfile.TemporaryDirectory(prefix=".tablecraft-msi-", dir=output) as temporary:
        temporary = Path(temporary)
        payload = temporary / "payload"
        stage_payload(version, target, binary, notices, payload)
        registration = temporary / "associations.wxi"
        write_registry(registration)
        snapshot_icon = temporary / "TableCraft.ico"
        shutil.copyfile(icon, snapshot_icon)
        built = temporary / installer.name
        subprocess.run(build_command(version, target, payload.resolve(), snapshot_icon.resolve(),
                                     registration.resolve(), built.resolve(), tool), check=True)
        verify(built, version, target, payload)
        created = []
        try:
            # Linking publishes without replacing any existing artifact. Staging
            # is a child of output, so it is on the same filesystem.
            os.link(built, installer)
            created.append(installer)
            with checksum.open("x", encoding="ascii", newline="\n") as stream:
                created.append(checksum)
                stream.write(f"{contract.digest(installer)}  {installer.name}\n")
        except BaseException:
            for path in reversed(created):
                path.unlink(missing_ok=True)
            raise
    return installer


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", choices=ARCHITECTURES, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--notices", type=Path, required=True)
    parser.add_argument("--icon", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--wix", default="wix")
    args = parser.parse_args()
    try:
        print(package(args.version, args.target, args.binary, args.notices, args.icon, args.output, args.wix))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Windows installer packaging failed: {error}\n")


if __name__ == "__main__":
    main()
