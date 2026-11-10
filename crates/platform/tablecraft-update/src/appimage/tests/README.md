# Inert AppImage signature carriers

These are first-party test data, created for TableCraft on 9 November 2026. They
contain a manually assembled 64-byte ELF64 little-endian x86-64 header, the Type 2
AppImage tag, and an ASCII version label. They contain no executable program or
filesystem and must never be executed. They are not native AppImage runtime tests.

`original.bin` models version 0.1.0 and `update.bin` models 0.1.1. Their Minisign
trusted comments bind the corresponding canonical Linux x64 AppImage asset name
and version. A fresh test-only key signed both with Minisign's prehashed Ed25519
mode. Its private key was deleted after generation. The public key is explicitly
rejected by production trust and packaging checks, even when its comment or key ID
changes. No upstream executable, fixture, signing key or license text was copied.

The tests authenticate source identity, reject unrelated runtime paths, validate
fresh source/payload bytes, prepare without execution, and exercise image-only
replacement/rollback with injected filesystem/helper outcomes. Native runtime
mounting, actual relaunch and real release-image signatures require native release
qualification separately.
