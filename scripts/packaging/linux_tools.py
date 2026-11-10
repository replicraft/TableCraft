#!/usr/bin/env python3
"""Fetch reviewed Linux tools and ship complete AppImage runtime source/relink material."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import uuid

import release_contract as contract

INVENTORY = contract.ROOT / "packaging/linux/tools.json"
MAX_INVENTORY_BYTES = 1024 * 1024
MAX_DOWNLOAD_BYTES = 64 * 1024 * 1024
MAX_NOTICE_BYTES = 1024 * 1024
MAX_KIT_BYTES = 128 * 1024 * 1024
CHUNK_BYTES = 1024 * 1024
HOSTS = {
    "api.github.com", "github.com", "raw.githubusercontent.com", "codeload.github.com",
    "release-assets.githubusercontent.com", "objects.githubusercontent.com",
    "musl.libc.org", "zlib.net",
}
FILES = {"linuxdeploy", "appimagetool", "runtime", "RuntimeThirdPartyLicenses.txt", "RuntimeSource.tar.gz"}
COMPONENT = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*\Z")
DIGEST = re.compile(r"[0-9a-f]{64}\Z")

REBUILD = r'''#!/bin/sh
# Explicitly invoked inside the source-only container; never run while inspecting a database.
set -eu
mkdir -p /work/runtime /work/fuse /work/squashfuse /out
cd /kit
tar -xzf sources/runtime.tar.gz -C /work/runtime --strip-components=1
if [ -n "${FUSE_SOURCE:-}" ]; then
    cp -R "/kit/$FUSE_SOURCE/." /work/fuse/
else
    tar -xJf sources/libfuse.tar.xz -C /work/fuse --strip-components=1
fi
patch -d /work/fuse -p1 < /work/runtime/patches/libfuse/mount.c.diff
if [ "${RELINK_PROOF:-0}" = 1 ]; then
    python3 - <<'PROOF'
from pathlib import Path
path = Path('/work/fuse/lib/mount.c')
source = path.read_text()
original = 'fuse: device not found'
if source.count(original) != 1:
    raise SystemExit('Cannot establish the independent FUSE relink change')
source = source.replace(original, 'TableCraft relink proof: fuse: device not found')
source += '\n/* TableCraft 2026-11-09: relink test changes one diagnostic above. */\n'
path.write_text(source)
PROOF
fi
meson setup /work/fuse/build /work/fuse --prefix=/usr --default-library=static \
    -Dexamples=false -Dtests=false -Duseroot=false
ninja -C /work/fuse/build -j "${JOBS:-2}" install
tar -xzf sources/squashfuse.tar.gz -C /work/squashfuse --strip-components=1
cd /work/squashfuse
./autogen.sh
CFLAGS='-ffunction-sections -fdata-sections -Os' ./configure LDFLAGS=-static
make -j "${JOBS:-2}"
make install
install -m 644 ./*.h /usr/local/include/squashfuse/
cd /work/runtime/src/runtime
printf '%s\n' 'https://github.com/AppImage/type2-runtime/commit/fedfad5' > version
make -j "${JOBS:-2}" runtime
strip --strip-debug --strip-unneeded runtime
# Match upstream's documented type-2 marker after linking/stripping.
printf 'AI\002' | dd of=runtime bs=1 count=3 seek=8 conv=notrunc
cp runtime /out/runtime
chmod 755 /out/runtime
apk info -v > /out/packages.txt
clang --version > /out/compiler.txt
'''

GUIDE = '''# AppImage runtime corresponding source and relink material

This kit accompanies the statically linked AppImage type-2 runtime. It contains
its complete upstream source/build files, the exact libfuse and squashfuse
sources, all other explicitly linked library sources, Alpine patches matching
the observed binary build, their original per-file notices, and compiler-runtime
license/exception texts. The notices file at the top level describes the chosen
license scopes. The complete original source archives preserve notices that
apply only to individual files as well as the top-level license statements.

The binary is the independently digest-pinned upstream asset in provenance.json;
this kit is not a claim of byte-identical compilation. Build logs identify musl,
zlib, zstd, mimalloc, patched libfuse and squashfuse. XZ/liblzma, LZO and LZ4 were
not enabled in that runtime build. Build tools themselves are not shipped in the
application. The companion Linux library notices cover separately deployed .so
files from the desktop's host-library closure.

## Rebuild or modify the LGPL library and relink the runtime

On a native Linux host with Docker, explicitly run:

    docker build --tag tablecraft-runtime-local --file Containerfile .
    container=$(docker create tablecraft-runtime-local)
    docker cp "$container:/out/runtime" ./runtime-rebuilt
    docker rm "$container"
    chmod +x runtime-rebuilt
    ./runtime-rebuilt --appimage-version

To edit libfuse first, unpack sources/libfuse.tar.xz into edits/fuse, modify its
sources, and add --build-arg FUSE_SOURCE=edits/fuse to docker build. The supplied
upstream mount patch is applied before compilation; adjust it if your changes
modify the same code. You may edit the build instructions as needed for your
changes. libfuse is compiled from that source and statically relinked into the
complete runtime source; there is no private object file, signing key, network
service or TableCraft database required to do so.

The container uses the pinned Alpine base and matching versioned static library
packages. Their complete corresponding sources/patches are also included under
sources/ and patches/. If an Alpine package mirror eventually removes one of
these versions, build those provided source versions on a compatible musl
system and point the ordinary compiler/linker paths at your builds. Original
Alpine recipe commit/path/hash identities are retained in provenance.json. A
source build needs normal system/compiler tools, not a reproduction of every
host build-tool package; no byte-reproducibility claim is made.

For an independently observable rebuild test, pass --build-arg RELINK_PROOF=1.
That changes one FUSE mount diagnostic before compiling. TableCraft's CI checks
that the changed diagnostic is present in the resulting runtime and that the
native version command succeeds; it does not substitute this test runtime for
the reviewed release asset.

## Recombine with the application filesystem

Using the reviewed appimagetool build tool, explicitly extract your existing
AppImage and reconstruct it with the rebuilt runtime:

    ./TableCraft.AppImage --appimage-extract
    appimagetool --runtime-file ./runtime-rebuilt ./squashfs-root ./TableCraft-relinked.AppImage
    chmod +x ./TableCraft-relinked.AppImage

This keeps the separate application payload while replacing its runtime. These
commands are optional source/relink instructions, never executed during normal
opening, inspection, or downloading of an application/database. A modified image
will not carry the publisher's signature for the original file.
'''


def safe_path(name):
    parts = name.split("/")
    if not parts or any(not COMPONENT.fullmatch(part) or part in (".", "..") for part in parts):
        raise ValueError("Tool inventory contains an unsafe output path")
    return name


def authorized(url):
    parsed = urllib.parse.urlsplit(url)
    return (parsed.scheme == "https" and parsed.hostname in HOSTS
            and parsed.port in (None, 443) and parsed.username is None
            and parsed.password is None and not parsed.fragment)


class Redirects(urllib.request.HTTPRedirectHandler):
    max_redirections = 5
    max_repeats = 2

    def redirect_request(self, request, response, code, message, headers, url):
        if not authorized(url):
            raise ValueError("Tool download attempted an unauthorized redirect")
        redirected = super().redirect_request(request, response, code, message, headers, url)
        if urllib.parse.urlsplit(request.full_url).hostname != urllib.parse.urlsplit(url).hostname:
            redirected.remove_header("Authorization")
        return redirected


def load_inventory(path=INVENTORY):
    data = contract.regular(path, MAX_INVENTORY_BYTES).read_bytes()
    metadata = json.loads(data)
    if metadata.get("schema") != 1 or set(metadata["targets"]) != {
        "x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu",
    }:
        raise ValueError("Unsupported Linux tool inventory")
    if len(metadata["sources"]) > 128:
        raise ValueError("Too many runtime source inputs")
    names = set()
    for item in metadata["sources"]:
        name = safe_path(item["path"])
        if name in names:
            raise ValueError("Duplicate runtime source input")
        names.add(name)
    for target in metadata["targets"].values():
        if set(target) != {"linuxdeploy", "appimagetool", "runtime"}:
            raise ValueError("Incomplete Linux tool set")
    for item in [*metadata["sources"], *(asset for target in metadata["targets"].values() for asset in target.values())]:
        if (not authorized(item["url"]) or not DIGEST.fullmatch(item["sha256"])
                or not isinstance(item["size"], int) or not 0 < item["size"] <= MAX_DOWNLOAD_BYTES):
            raise ValueError("Invalid pinned Linux tool/source identity")
    return metadata, hashlib.sha256(data).hexdigest()


def store_checked(stream, destination, item):
    destination.parent.mkdir(parents=True, exist_ok=True)
    created = False
    digest = hashlib.sha256()
    secondary = hashlib.sha512() if item.get("sha512") else None
    try:
        with destination.open("xb") as output:
            created = True
            received = 0
            while block := stream.read(min(CHUNK_BYTES, item["size"] - received + 1)):
                received += len(block)
                if received > item["size"]:
                    raise ValueError("Tool/source payload exceeds its pinned length")
                output.write(block)
                digest.update(block)
                if secondary is not None:
                    secondary.update(block)
        if received != item["size"] or digest.hexdigest() != item["sha256"]:
            raise ValueError("Tool/source payload does not match its pinned digest/length")
        if secondary is not None and secondary.hexdigest() != item["sha512"]:
            raise ValueError("Runtime source differs from the published Alpine SHA-512")
    except BaseException:
        if created:
            destination.unlink(missing_ok=True)
        raise


def download(item, destination, opener=None):
    if not authorized(item["url"]):
        raise ValueError("Unauthorized tool URL")
    headers = {"User-Agent": "TableCraft-packaging", "Accept-Encoding": "identity", "Accept": "application/octet-stream"}
    if urllib.parse.urlsplit(item["url"]).hostname == "api.github.com" and os.environ.get("GH_TOKEN"):
        headers["Authorization"] = "Bearer " + os.environ["GH_TOKEN"]
    request = urllib.request.Request(item["url"], headers=headers)
    opener = opener or urllib.request.build_opener(Redirects())
    with opener.open(request, timeout=60) as response:
        if response.status != 200:
            raise ValueError("Tool download returned an unexpected status")
        advertised = response.headers.get("Content-Length")
        if advertised is not None and int(advertised) != item["size"]:
            raise ValueError("Tool download advertised the wrong length")
        store_checked(response, destination, item)


def runtime_notices(metadata, sources):
    sections = [
        "AppImage runtime and statically linked components\n",
        "Complete original source, patches, per-file copyright notices and relink instructions accompany this image in RuntimeSource.tar.gz.\n",
        "libfuse is the LGPL-2.1 library, not merely the MIT runtime wrapper. Source is provided with the image; no future source offer is substituted.\n",
    ]
    for entry in metadata["licenses"]:
        sections.append(f"\n{entry['component']}\nLicense scope: {entry['license']}\n")
        with tarfile.open(sources / entry["source"]) as archive:
            for name in entry["members"]:
                member = archive.getmember(name)
                if not member.isfile() or not 0 < member.size <= MAX_NOTICE_BYTES:
                    raise ValueError("Runtime license member is not bounded regular text")
                with archive.extractfile(member) as stream:
                    text = stream.read(MAX_NOTICE_BYTES + 1).decode("utf-8")
                sections.append(f"\nOriginal source notice: {name}\n{text}\n")
    for name in ("licenses/GCC-COPYING3.txt", "licenses/GCC-COPYING.RUNTIME.txt"):
        sections.append("\nAlpine compiler-runtime support licensing\n" + (sources / name).read_text("utf-8"))
    text = "\n".join(sections).encode("utf-8")
    if len(text) > contract.MAX_LEGAL_BYTES:
        raise ValueError("Runtime notices exceed the legal-payload limit")
    return text


def containerfile(metadata):
    versions = metadata["runtime_build"]["packages"]
    packages = ["bash", "alpine-sdk", "autoconf", "automake", "libtool", "meson", "ninja", "xz", "eudev-dev", "gettext-dev", "linux-headers", "clang", "python3"]
    for name in ("musl-dev", "zstd-dev", "zstd-static", "zlib-dev", "zlib-static", "mimalloc2-dev"):
        key = name.split("-", 1)[0]
        packages.append(name + "=" + versions[key])
    return ("FROM " + metadata["runtime_build"]["base_image"] + "\n"
            + "RUN apk add --no-cache " + " \\\n    ".join(packages) + "\n"
            + "COPY . /kit\nARG FUSE_SOURCE=\nARG RELINK_PROOF=0\n"
            + "RUN FUSE_SOURCE=\"$FUSE_SOURCE\" RELINK_PROOF=\"$RELINK_PROOF\" sh /kit/rebuild-runtime.sh\n")


def source_kit(metadata, target, sources, notices, output):
    contents = {item["path"]: sources / item["path"] for item in metadata["sources"]}
    contents.update({
        "README.md": GUIDE.encode(), "Containerfile": containerfile(metadata).encode(),
        "rebuild-runtime.sh": REBUILD.encode(), "RuntimeThirdPartyLicenses.txt": notices,
        "provenance.json": (json.dumps({"target": target, "inventory": metadata}, indent=2, sort_keys=True) + "\n").encode(),
    })
    with output.open("xb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", mtime=0, filename="") as compressed:
        with tarfile.open(fileobj=compressed, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for name, value in sorted(contents.items()):
                safe_path(name)
                member = tarfile.TarInfo("RuntimeSource/" + name)
                member.mode = 0o755 if name.endswith(".sh") else 0o644
                member.size = value.stat().st_size if isinstance(value, Path) else len(value)
                if isinstance(value, Path):
                    with value.open("rb") as stream:
                        archive.addfile(member, stream)
                else:
                    archive.addfile(member, io.BytesIO(value))
    contract.regular(output, MAX_KIT_BYTES)


def verify_source_kit(path, notices, metadata, target):
    inputs = {"RuntimeSource/" + item["path"]: item for item in metadata["sources"]}
    generated = {
        "RuntimeSource/README.md": len(GUIDE.encode()),
        "RuntimeSource/Containerfile": len(containerfile(metadata).encode()),
        "RuntimeSource/rebuild-runtime.sh": len(REBUILD.encode()),
        "RuntimeSource/provenance.json": MAX_INVENTORY_BYTES,
        "RuntimeSource/RuntimeThirdPartyLicenses.txt": contract.MAX_LEGAL_BYTES,
    }
    expected = set(inputs) | set(generated)
    seen = set()
    total = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-runtime-source-check-") as temporary:
        root = Path(temporary)
        with tarfile.open(contract.regular(path, MAX_KIT_BYTES), "r:gz") as archive:
            for member in archive:
                if member.name not in expected or member.name in seen or not member.isfile():
                    raise ValueError("Runtime source kit has unexpected or repeated members")
                seen.add(member.name)
                total += member.size
                maximum = inputs[member.name]["size"] if member.name in inputs else generated[member.name]
                if member.size <= 0 or member.size > maximum or total > MAX_KIT_BYTES:
                    raise ValueError("Runtime source kit exceeds its bounded source inventory")
                if member.name in inputs:
                    item = inputs[member.name]
                    with archive.extractfile(member) as stream:
                        store_checked(stream, root / item["path"], item)
        if seen != expected:
            raise ValueError("Runtime source kit is incomplete")
        expected_notices = runtime_notices(metadata, root)
        if contract.regular(notices, contract.MAX_LEGAL_BYTES).read_bytes() != expected_notices:
            raise ValueError("Runtime notices differ from the reviewed source inputs")
        # Rebuild the canonical outer archive from authenticated source bytes and
        # current first-party glue. A rewritten receipt cannot authorize new code.
        canonical = root / "RuntimeSource.tar.gz"
        source_kit(metadata, target, root, expected_notices, canonical)
        if contract.digest(canonical) != contract.digest(path):
            raise ValueError("Runtime source kit differs from its canonical reviewed material")


def verify_binary(path, item, target):
    contract.regular(path, MAX_DOWNLOAD_BYTES)
    if path.stat().st_size != item["size"] or contract.digest(path) != item["sha256"]:
        raise ValueError("Linux tool differs from its maintained upstream digest")
    with path.open("rb") as stream:
        contract.architecture(stream, path.stat().st_size, target)
        stream.seek(8)
        if stream.read(3) != b"AI\x02":
            raise ValueError("Linux tool/runtime is not a type-2 AppImage carrier")


def verify_cache(output, target, fingerprint, metadata):
    if output.is_symlink() or not output.is_dir():
        raise ValueError("Linux tool cache must be a regular owned directory")
    receipt = json.loads(contract.regular(output / "receipt.json", MAX_INVENTORY_BYTES).read_bytes())
    generator = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    if (receipt.get("target") != target or receipt.get("inventory_sha256") != fingerprint
            or receipt.get("generator_sha256") != generator or set(receipt["files"]) != FILES):
        raise ValueError("Linux tool cache belongs to another inventory/target/generator")
    allowed = FILES | {"receipt.json", "relink-check.json"}
    if not {path.name for path in output.iterdir()} <= allowed:
        raise ValueError("Linux tool cache contains unowned files")
    for name, expected in receipt["files"].items():
        if contract.digest(contract.regular(output / name, MAX_KIT_BYTES)) != expected:
            raise ValueError("Linux tool cache payload changed")
    for name, item in metadata["targets"][target].items():
        verify_binary(output / name, item, target)
    verify_source_kit(output / "RuntimeSource.tar.gz", output / "RuntimeThirdPartyLicenses.txt", metadata, target)
    if (output / "relink-check.json").exists() or (output / "relink-check.json").is_symlink():
        contract.regular(output / "relink-check.json", MAX_INVENTORY_BYTES)
    return receipt


def fetch(target, output, inventory=INVENTORY, opener=None):
    metadata, fingerprint = load_inventory(inventory)
    if target not in metadata["targets"]:
        raise ValueError("Unsupported Linux tool target")
    if output.exists() or output.is_symlink():
        return verify_cache(output, target, fingerprint, metadata)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".tablecraft-linux-tools-", dir=output.parent) as temporary:
        stage = Path(temporary)
        sources = stage / "source-inputs"
        for item in metadata["sources"]:
            download(item, sources / item["path"], opener)
        for name, item in metadata["targets"][target].items():
            download(item, stage / name, opener)
            verify_binary(stage / name, item, target)
            (stage / name).chmod(0o755)
        notices = runtime_notices(metadata, sources)
        (stage / "RuntimeThirdPartyLicenses.txt").write_bytes(notices)
        source_kit(metadata, target, sources, notices, stage / "RuntimeSource.tar.gz")
        receipt = {
            "schema": 1, "target": target, "inventory_sha256": fingerprint,
            "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "files": {name: contract.digest(stage / name) for name in sorted(FILES)},
            "assets": metadata["targets"][target], "runtime_build": metadata["runtime_build"],
        }
        (stage / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
        created = []
        output.mkdir(exist_ok=False)
        try:
            for name in sorted(FILES | {"receipt.json"}):
                with (stage / name).open("rb") as source, (output / name).open("xb") as destination:
                    created.append(output / name)
                    shutil.copyfileobj(source, destination, CHUNK_BYTES)
                (output / name).chmod(0o755 if name in ("runtime", "linuxdeploy", "appimagetool") else 0o644)
        except BaseException:
            for path in created:
                path.unlink(missing_ok=True)
            output.rmdir()
            raise
    return receipt


def relink(target, output, inventory=INVENTORY):
    if sys.platform != "linux":
        raise ValueError("The explicit runtime relink check requires native Linux and Docker")
    metadata, fingerprint = load_inventory(inventory)
    receipt = verify_cache(output, target, fingerprint, metadata)
    if os.uname().machine != {"x86_64-unknown-linux-gnu": "x86_64", "aarch64-unknown-linux-gnu": "aarch64"}[target]:
        raise ValueError("Runtime relink checks must execute on the native target")
    evidence_path = output / "relink-check.json"
    if evidence_path.exists() or evidence_path.is_symlink():
        raise ValueError("Relink evidence requires a fresh output path")
    with tempfile.TemporaryDirectory(prefix="tablecraft-runtime-relink-") as temporary:
        root = Path(temporary)
        with tarfile.open(output / "RuntimeSource.tar.gz", "r:gz") as kit:
            for member in kit:
                safe_path(member.name)
                if not member.isfile() or member.size > MAX_DOWNLOAD_BYTES:
                    raise ValueError("Runtime kit contains an invalid outer source member")
                path = root / member.name
                path.parent.mkdir(parents=True, exist_ok=True)
                with kit.extractfile(member) as source, path.open("xb") as destination:
                    shutil.copyfileobj(source, destination, CHUNK_BYTES)
        tag = "tablecraft-runtime-relink-" + uuid.uuid4().hex
        container = None
        built = False
        environment = os.environ.copy()
        environment.pop("GH_TOKEN", None)
        try:
            subprocess.run(["docker", "build", "--tag", tag, "--build-arg", "RELINK_PROOF=1", "--file", str(root / "RuntimeSource/Containerfile"), str(root / "RuntimeSource")], check=True, env=environment, timeout=1800)
            built = True
            container = subprocess.run(["docker", "create", tag, "sh"], check=True, capture_output=True, text=True, env=environment).stdout.strip()
            if not re.fullmatch(r"[0-9a-f]{64}", container):
                raise ValueError("Docker did not return an owned container identity")
            rebuilt = root / "runtime-rebuilt"
            subprocess.run(["docker", "cp", container + ":/out/runtime", str(rebuilt)], check=True, env=environment)
            with rebuilt.open("rb") as stream:
                contract.architecture(stream, rebuilt.stat().st_size, target)
            build_packages = root / "packages.txt"
            compiler = root / "compiler.txt"
            for source, destination in (("packages.txt", build_packages), ("compiler.txt", compiler)):
                subprocess.run(
                    ["docker", "cp", container + ":/out/" + source, str(destination)],
                    check=True, env=environment, timeout=60,
                )
            packages_text = contract.regular(build_packages, MAX_NOTICE_BYTES).read_text("utf-8")
            compiler_text = contract.regular(compiler, MAX_NOTICE_BYTES).read_text("utf-8")
            for name, version in metadata["runtime_build"]["packages"].items():
                package_name = {"musl": "musl-dev", "zstd": "zstd-static", "zlib": "zlib-static", "mimalloc2": "mimalloc2-dev"}[name]
                if package_name + "-" + version not in packages_text.splitlines():
                    raise ValueError("The runtime rebuild used unexpected static library packages")
            data = rebuilt.read_bytes()
            if data[8:11] != b"AI\x02" or b"TableCraft relink proof:" not in data:
                raise ValueError("The runtime did not incorporate the changed FUSE library")
            result = subprocess.run([str(rebuilt), "--appimage-version"], check=True, capture_output=True, text=True, timeout=30)
            version = result.stdout + result.stderr
            if metadata["runtime_build"]["commit"][:7] not in version:
                raise ValueError("The rebuilt runtime did not report its pinned source identity")
            evidence = {
                "schema": 1, "target": target, "result": "passed",
                "source_kit_sha256": receipt["files"]["RuntimeSource.tar.gz"],
                "rebuilt_runtime_sha256": contract.digest(rebuilt),
                "modified_fuse_diagnostic": True, "native_version_command": True,
                "installed_packages": packages_text.splitlines(),
                "compiler": compiler_text,

            }
            with evidence_path.open("x", encoding="utf-8") as stream:
                stream.write(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
        finally:
            if container and re.fullmatch(r"[0-9a-f]{64}", container):
                subprocess.run(["docker", "rm", "--force", container], check=True, env=environment)
            if built:
                subprocess.run(["docker", "image", "rm", tag], check=True, env=environment)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("fetch", "verify-relink"))
    parser.add_argument("--target", choices=("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "fetch":
            fetch(args.target, args.output)
        else:
            relink(args.target, args.output)
        print(args.output)
    except (OSError, ValueError, KeyError, TypeError, urllib.error.URLError, subprocess.SubprocessError, tarfile.TarError) as error:
        parser.exit(1, f"Linux tool preparation failed: {error}\n")


if __name__ == "__main__":
    main()
