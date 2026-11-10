# Linux desktop packages

`package_linux.py` prepares desktop-only DEB, RPM and AppImage files on native
x86-64 or ARM64 Linux. The version/architecture names come from the shared
installer contract. The CLI remains a separate portable download.

The native build uses the same desktop executable, generated 256-pixel icon and
reviewed desktop notices as the portable archive. DEB/RPM install their payload
and `RELEASE.json` under `/usr/lib/tablecraft`, with `/usr/bin/tablecraft-desktop`
as a launcher link. The existing association codec supplies MDB/ACCDB desktop
and MIME metadata; installation/removal refresh caches without choosing a default
handler. Icons, application metadata and both project/third-party notices are
included. Package-manager installations require package-manager updates.

DEB dependencies combine actual `dpkg-shlibdeps` symbol-version results with the
X11, Wayland, graphics and Fontconfig libraries opened dynamically by enabled
backends. RPM keeps automatic ELF dependencies and adds the corresponding
runtime sonames. RPM stripping is disabled so installed bytes retain their
captured manifest hashes. The script verifies native metadata and complete
payload bytes/digests before publishing each package and its SHA-256 sidecar.
The packaging maintainer label is Replicraft, matching the existing project
publisher; this is not a claim of acceptance into a distribution repository.

AppImage uses linuxdeploy for dependency/RPATH handling and appimagetool for its
container. Both tools and a matching reviewed type-2 runtime are explicit inputs;
the packager does not download an implicit latest runtime. The AppDir keeps the
manifest beside `usr/bin/tablecraft-desktop`, regenerates its executable hash
after linuxdeploy modifies RPATH, and uses a small first-party AppRun wrapper.
Bundled native libraries retain package/version attribution, original/deployed
hashes, their distribution copyright texts and referenced common licenses.
A separately reviewed AppImage-runtime notice bundle is mandatory. GPU drivers,
the display server/compositor and excluded base-system libraries remain host
requirements. An AppImage is not a fully static executable.

The current updater must treat an AppImage mount as manual-only until an actual
AppImage replacement service is implemented: `$APPDIR` is the temporary mounted
AppDir and `$APPIMAGE` identifies the original outer file. A mounted `usr/bin`
payload must never be mistaken for a writable portable installation.

Example inputs on the matching native build host:

```sh
python scripts/packaging/package_linux.py --version 0.1.0 \
  --target x86_64-unknown-linux-gnu \
  --binary target/x86_64-unknown-linux-gnu/release/tablecraft-desktop \
  --notices resources/DesktopThirdPartyLicenses.txt \
  --icon resources/icons/application-256.png --output dist \
  --linuxdeploy .cache/linux-tools/linuxdeploy \
  --appimagetool .cache/linux-tools/appimagetool \
  --appimage-runtime .cache/linux-tools/runtime \
  --appimage-runtime-notices .cache/linux-tools/RuntimeThirdPartyLicenses.txt
```

The native job additionally installs/removes the DEB on its disposable runner and
runs only extracted/installed `--help` and `--version`; it opens no window or
database. RPM metadata/payload checks are distinct from an actual RPM-distribution
installation. Local macOS preparation/refusal tests do not establish any native
Linux package success.

Primary references inspected for the implementation:

- [linuxdeploy native deployment guide](https://docs.appimage.org/packaging-guide/from-source/native-binaries.html)
- [appimagetool options and explicit runtime input](https://github.com/AppImage/appimagetool/blob/main/README.md)
- [AppDir layout](https://docs.appimage.org/packaging-guide/manual.html)
- [dpkg-shlibdeps dependency generation](https://manpages.debian.org/bookworm/dpkg-dev/dpkg-shlibdeps.1.en.html)
- [Debian control fields](https://www.debian.org/doc/debian-policy/ch-controlfields.html)
- [RPM spec and automatic dependencies](https://rpm.org/docs/6.0.x/manual/spec.html)

PaintCraft's existing Linux packaging was inspected as a workflow reference.
No source code, executable, icon or fixture was copied from it.
