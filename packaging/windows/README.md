# Windows desktop installer

`tablecraft.wxs` builds a desktop-only MSI for x64 or ARM64 with WiX 6. The CLI
retains its separate release archives. The installer owns four files under the
machine's 64-bit Program Files directory: the desktop executable, project license,
consolidated notices and `RELEASE.json`. Its stable TableCraft-specific UpgradeCode
supports native major upgrades with old-product removal inside the installation
transaction. There are no application-launch custom actions.

The packager snapshots inputs, validates the PE architecture and real ICO pixel
records, generates the shared registration include, builds the MSI and then reads
its actual Windows Installer tables. It checks product/architecture/component
ownership, the Start Menu shortcut, upgrade rows and embedded icon. A quiet
administrative extraction must reproduce every expected payload hash before the
MSI and checksum are published without overwriting existing outputs.

```powershell
python scripts/packaging/package_windows.py --version 0.1.0 `
  --target x86_64-pc-windows-msvc --binary target/release/tablecraft-desktop.exe `
  --notices resources/DesktopThirdPartyLicenses.txt `
  --icon resources/icons/application.ico --output dist
```

Application registration derives from `file_associations.windows_records` and
retains its existing per-extension ProgIDs and capabilities. MSI installs those
records machine-wide and uses empty REG_SZ for OpenWithProgids, which Microsoft
explicitly permits alongside REG_NONE. It never changes extension default values
or UserChoice. The quoted open command supplies one literal filename after `--`
to the existing desktop argument parser. Running-instance forwarding remains
unimplemented; these registrations do not establish IPC support.

On a clean, disposable, elevated Windows CI runner, run the separate lifecycle
check after packaging:

```powershell
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass `
  -File packaging/windows/test_lifecycle.ps1 `
  -Installer dist/tablecraft-desktop-0.1.0-windows-x64.msi `
  -Version 0.1.0 -Target x86_64-pc-windows-msvc
```

It quietly installs into a temporary owned directory, checks installed hashes,
runs only `--version`, verifies registry/shortcut receivers and unchanged existing
association defaults, then uninstalls and checks user database preservation. No
desktop window or database content is opened. Existing TableCraft installations
or registrations cause a refusal instead of being modified by the test. Logs are
retained in the reported temporary evidence directory. This script is not run on
a developer's machine automatically.

Minisign release authentication and Windows Authenticode are distinct. This
packager does not Authenticode-sign the executable or MSI. The updater currently
classifies Program Files installations as manual; automatic signed-MSI upgrade
is a separate integration requirement. Host-side synthetic/preflight tests are
not evidence of a working native MSI build or installation.

References inspected for this implementation:

- The user's local PaintCraft Windows packaging layout and native WiX approach.
- [WiX MajorUpgrade](https://docs.firegiant.com/wix/schema/wxs/majorupgrade/).
- [WiX registry value types](https://docs.firegiant.com/wix/schema/wxs/registryvaluetype/).
- [Microsoft Open With registration](https://learn.microsoft.com/en-us/windows/win32/shell/how-to-include-an-application-on-the-open-with-dialog-box).
