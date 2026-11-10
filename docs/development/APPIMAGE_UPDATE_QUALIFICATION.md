# Headless native AppImage update qualification

The Linux desktop packaging jobs run
[`appimage_update_qualification.py`](../../scripts/packaging/appimage_update_qualification.py)
on both native architectures. The hook is implemented; a passing native CI receipt
is still required. Local macOS refusal/receipt tests and compilation do not qualify
Linux execution.

The harness copies the real built release AppImage outside `dist`, checks its
actual runtime with `--appimage-extract-and-run --version`, and extracts its real
AppDir. It creates a separate newer-version **headless receiver fixture** using the
reviewed native runtime and AppImage tool. This receiver is a small first-party
Rust binary with no GUI/window/rendering code. The candidate is not a released
TableCraft desktop build and never enters the release artifact set.

A fresh CI-only Minisign key signs the original and candidate, binding each exact
product/version/architecture in its trusted comment. The public key is explicitly
marked test-only; its private half is deleted immediately after signing. No
production key, trust override, alternate network endpoint or shipped engineering
feature is introduced. Both architectures use the same pinned official Minisign
archive, selecting and checking their own ELF member before execution.

One explicitly ignored updater test receives only these isolated fixtures. It
uses crate-private source qualification against the real extracted executable,
manifest and signed original image, then the ordinary verified download,
preparation and apply services. The test process becomes the helper's actual
parent. It observes that the helper retains the original and does not relaunch
while that parent remains alive, writes its handoff receipt, and exits normally.

The outer harness then checks the real helper and receiver on Linux:

- Successful headless launch uses the original image pathname, gets fresh runtime
  environment values and retires the exact private rollback backup.
- An immediately failing receiver leaves the original image and READY marker
  available in the private backup after the helper exits.
- An unrelated database remains unchanged; the actual release AppImage in `dist`
  retains its original SHA-256.

Test images, keys, AppDirs and retained failure fixtures are removed only from the
harness's private temporary tree after evidence collection. Raw logs and JSON
receipts have their own Linux evidence artifact and are excluded from release
assembly. They record that GUI relaunch is unqualified.

This establishes actual Type 2 extraction/runtime operation and a headless
normal-exit handoff. It does not establish a GUI restart, unsaved-document behavior
inside a running AppImage, FUSE-mounted execution, or a newer production desktop
binary. The source-proof test explicitly supplies the extracted executable path;
it does not pretend the unit-test binary is the desktop process. Existing shared
host tests cover document/worker gates separately. See
[the signed updater contract](SELF_UPDATE.md) for production ownership and remaining
release gates.
