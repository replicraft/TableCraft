#!/usr/bin/env python3
"""Native Linux headless qualification, isolated from release assets and trust."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import time

import package_desktop as desktop
import package_linux as linux
import release_contract as contract

TEST = "appimage::installed::native::native_actual_image_and_headless_receiver_wait_for_normal_parent_exit"
RECEIVER = Path(__file__).with_name("appimage_receiver.rs")
MAX_LOG_BYTES = 8 * 1024 * 1024
WAIT_SECONDS = 45


def environment():
    values = os.environ.copy()
    for name in ("UPDATE_SIGNING_KEY", "UPDATE_SIGNING_PASSWORD", "TABLECRAFT_UPDATE_SIGNING_KEY",
                 "TABLECRAFT_UPDATE_SIGNING_PASSWORD", "APPDIR", "APPIMAGE", "ARGV0", "OWD"):
        values.pop(name, None)
    return values


def run(command, log, *, cwd=None, env=None, timeout=600):
    with log.open("xb") as output:
        result = subprocess.run([str(item) for item in command], cwd=cwd,
                                env=environment() if env is None else env,
                                stdin=subprocess.DEVNULL, stdout=output,
                                stderr=subprocess.STDOUT, timeout=timeout, check=False)
    if log.stat().st_size > MAX_LOG_BYTES:
        raise ValueError("Native qualification log exceeded its bound")
    if result.returncode:
        raise ValueError(f"Native qualification failed: {log.name} ({result.returncode})")


def process_alive(pid):
    try:
        record = Path(f"/proc/{pid}/stat").read_text("utf-8")
    except FileNotFoundError:
        return False
    # The process name may contain spaces or parentheses. Fields following its
    # final ')' start with state; a zombie has finished running the helper.
    state = record[record.rfind(")") + 2:].split()[0]
    return state != "Z"


def wait_for_exit(pid):
    deadline = time.monotonic() + WAIT_SECONDS
    while process_alive(pid):
        if time.monotonic() >= deadline:
            raise ValueError("Native qualification process did not reach a terminal state")
        time.sleep(0.1)


def receiver_receipt(path, version, image, previous_appdir):
    fields = {}
    for line in contract.regular(path, 16 * 1024).read_text("utf-8").splitlines():
        key, separator, value = line.partition("=")
        if not separator or key in fields:
            raise ValueError("Malformed headless receiver receipt")
        fields[key] = value
    if fields.get("receiver") != "tablecraft-headless-update-receiver" or fields.get("version") != version:
        raise ValueError("The expected headless receiver did not run")
    if Path(fields.get("APPIMAGE", "")).resolve() != image.resolve():
        raise ValueError("Receiver did not execute through the original AppImage pathname")
    appdir = Path(fields.get("APPDIR", ""))
    if not appdir.is_absolute() or appdir == previous_appdir:
        raise ValueError("The previous runtime environment leaked into relaunch")
    if fields.get("ARGV0") == "stale-appimage-argument" or fields.get("OWD") == "stale-appimage-working-directory":
        raise ValueError("The previous runtime identity survived helper relaunch")
    return fields


def sign(tool, secret, public, image, version, target, log):
    # Only this disposable key is accepted. Production secrets/endpoints and the
    # maintained release public key are never supplied to the qualification test.
    name = f"tablecraft-desktop-{version}-{contract.TARGETS[target]}.AppImage"
    run([tool, "-S", "-s", secret, "-m", image, "-t", f"tablecraft-update {version} {name}"], log)
    run([tool, "-V", "-q", "-p", public, "-m", image], log.with_name(log.stem + "-verify.log"))


def qualify(image, version, target, tool, appimagetool, runtime, evidence):
    contract.validate_version(version)
    if platform.system() != "Linux" or target not in linux.TARGETS or platform.machine() != linux.TARGETS[target][0]:
        raise ValueError("AppImage qualification requires the matching native Linux runner")
    contract.regular(image, contract.MAX_EXECUTABLE_BYTES)
    for binary in (tool, appimagetool, runtime):
        contract.regular(binary)
    if evidence.resolve().is_relative_to(image.resolve().parent):
        raise ValueError("Updater evidence must remain outside the release asset directory")
    evidence.mkdir(parents=True, exist_ok=False)
    original_digest = contract.digest(image)
    major, minor, patch = (int(number) for number in version.split("-", 1)[0].split("."))
    next_version = f"{major}.{minor}.{patch + 1}"
    reports = []

    with tempfile.TemporaryDirectory(prefix="tablecraft-appimage-qualification-") as temporary:
        root = Path(temporary)
        original = root / "original.AppImage"
        shutil.copy2(image, original)
        base_env = environment()
        base_env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
        run([original, "--appimage-extract-and-run", "--version"], evidence / "original-runtime-version.log", env=base_env)
        if (evidence / "original-runtime-version.log").read_text("utf-8").strip() != f"tablecraft-desktop {version}":
            raise ValueError("Actual AppImage runtime version mismatch")
        extraction = root / "extraction"
        extraction.mkdir()
        run([original, "--appimage-extract"], evidence / "original-extract.log", cwd=extraction, env=base_env)
        appdir = extraction / "squashfs-root"
        receiver = root / "headless-receiver"
        receiver_env = environment()
        receiver_env["TABLECRAFT_TEST_RECEIVER_VERSION"] = next_version
        run(["rustc", "--edition", "2024", "-O", RECEIVER, "-o", receiver], evidence / "receiver-build.log", env=receiver_env)
        candidate_appdir = root / "receiver.AppDir"
        shutil.copytree(appdir, candidate_appdir, symlinks=True)
        payload = candidate_appdir / "usr/bin"
        shutil.copy2(receiver, payload / linux.BINARY)
        files = {name: payload / name for name in (linux.BINARY, "LICENSE", desktop.NOTICES)}
        (payload / desktop.MANIFEST).write_bytes(desktop.manifest(next_version, target, files))
        candidate = root / "candidate.AppImage"
        build_env = dict(base_env, ARCH=linux.TARGETS[target][0], VERSION=next_version)
        run(
            [appimagetool.resolve(), "--runtime-file", runtime.resolve(), candidate_appdir, candidate],
            evidence / "receiver-appimage-build.log", env=build_env,
        )
        candidate.chmod(0o755)
        run([candidate, "--appimage-extract-and-run", "--version"], evidence / "receiver-runtime-version.log", env=base_env)
        observed_version = (evidence / "receiver-runtime-version.log").read_text("utf-8").strip()
        if observed_version != f"tablecraft-headless-update-receiver {next_version}":
            raise ValueError("The candidate is not the explicit headless receiver")
        public = root / "TEST-ONLY.pub"
        secret = root / "TEST-ONLY.key"
        run([tool, "-G", "-W", "-p", public, "-s", secret], evidence / "test-key-generation.log")
        secret.chmod(0o600)
        lines = public.read_text("utf-8").splitlines()
        public.write_text(
            "untrusted comment: TEST ONLY disposable AppImage qualification key\n" + lines[1] + "\n",
            encoding="utf-8",
        )
        sign(tool, secret, public, original, version, target, evidence / "original-sign.log")
        sign(tool, secret, public, candidate, next_version, target, evidence / "candidate-sign.log")
        secret.unlink()

        for fail in (False, True):
            case = root / ("failed-relaunch" if fail else "successful-relaunch")
            case.mkdir()
            for name in (
                "original.AppImage", "original.AppImage.minisig",
                "candidate.AppImage", "candidate.AppImage.minisig", "TEST-ONLY.pub",
            ):
                shutil.copy2(root / name, case / name)
            runtime_tree = case / "original-runtime"
            runtime_tree.mkdir()
            shutil.copytree(appdir, runtime_tree / "squashfs-root", symlinks=True)
            previous_appdir = runtime_tree / "squashfs-root"
            inputs = {
                "current": version,
                "next": next_version,
                "target": target,
                "qualification_receiver": "tablecraft-headless-update-receiver",
            }
            (case / "inputs.json").write_text(json.dumps(inputs), encoding="utf-8")
            (case / "private.accdb").write_bytes(b"untouched qualification database")
            case_env = dict(base_env, TABLECRAFT_TEST_APPIMAGE_ROOT=str(case),
                            TABLECRAFT_TEST_APPIMAGE_RECEIPT=str(case / "receiver.txt"),
                            APPDIR=str(previous_appdir), APPIMAGE=str(case / "original.AppImage"),
                            ARGV0="stale-appimage-argument", OWD="stale-appimage-working-directory")
            if fail:
                case_env["TABLECRAFT_TEST_APPIMAGE_FAIL"] = "1"
            log_name = "failed-relaunch.log" if fail else "successful-relaunch.log"
            run(
                ["cargo", "test", "--locked", "-p", "tablecraft-update", TEST,
                 "--", "--ignored", "--exact", "--nocapture"],
                evidence / log_name, cwd=contract.ROOT, env=case_env, timeout=1200,
            )
            handoff = json.loads((case / "handoff.json").read_text("utf-8"))
            wait_for_exit(handoff["helper_pid"])
            fields = receiver_receipt(case / "receiver.txt", next_version, case / "original.AppImage", previous_appdir)
            backup = Path(handoff["backup"])
            if fail:
                if (not backup.is_relative_to(case) or not (backup / "READY").is_file()
                        or contract.digest(backup / "original.AppImage") != original_digest):
                    raise ValueError("Failed relaunch lost its retained original")
            elif backup.exists():
                raise ValueError("Successful headless launch did not retire its private original backup")
            if (case / "private.accdb").read_bytes() != b"untouched qualification database":
                raise ValueError("AppImage replacement touched an unrelated database")
            wait_for_exit(int(fields["pid"]))
            reports.append({"failed_receiver": fail, "receiver": fields, "native_handoff": handoff,
                            "original_retained_after_failure": fail, "gui_relaunch_qualified": False})

    if contract.digest(image) != original_digest:
        raise ValueError("Qualification changed the actual release artifact")
    receipt = {
        "target": target,
        "version": version,
        "next_receiver_fixture_version": next_version,
        "release_image_sha256": original_digest,
        "release_unchanged": True,
        "fixtures_removed_after_evidence": True,
        "cases": reports,
    }
    (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("image", "minisign", "appimagetool", "runtime", "evidence"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True, choices=linux.TARGETS)
    args = parser.parse_args()
    try:
        qualify(
            args.image.resolve(), args.version, args.target, args.minisign.resolve(),
            args.appimagetool.resolve(), args.runtime.resolve(), args.evidence.resolve(),
        )
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"AppImage update qualification failed: {error}\n")


if __name__ == "__main__":
    main()
