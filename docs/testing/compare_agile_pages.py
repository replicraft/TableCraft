"""Compare explicit Agile password verification and data-page digests with Jackcess Encrypt."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
from compare_encryption_envelopes import CASES


RECIPES = Path(__file__).resolve().parent
# Public synthetic fixture passwords published in upstream CryptCodecProviderTest.
PASSWORDS = {"db2007-enc.accdb": "Test123", "db2013-enc.accdb": "1234"}


def run(command, password):
    payload = b"" if password is None else password.encode("utf-8") + b"\n"
    return subprocess.run([str(p) for p in command], input=payload, capture_output=True, check=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "crypt-jar", "bc-jar", "reader", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    classpath = os.pathsep.join(str(p) for p in [args.jar, args.crypt_jar, args.bc_jar])
    for name, password in PASSWORDS.items():
        path = args.directory / name
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        assert before == CASES[name][0], (name, "original source")
        reference = run(["java", "-cp", classpath, RECIPES / "EncryptedPagesOracle.java", path], password)
        assert reference.returncode == 0 and not reference.stderr, (name, reference.stderr)
        result = run([args.reader, path], password)
        assert result.returncode == 0 and not result.stderr, (name, result.stderr)
        assert result.stdout == reference.stdout, (name, "all decoded page hashes")
        rows = result.stdout.splitlines()
        assert len(rows) == path.stat().st_size // 4096 - 1
        for supplied, message in [(None, b"password is required"), ("wrong-public-test-password", b"incorrect database password"), ("", b"incorrect database password")]:
            failed = run([args.reader, path], supplied)
            assert failed.returncode == 2 and not failed.stdout and message in failed.stderr, (name, "credential refusal", failed.stderr)
        verified = run([args.reader, path, "--verify-only"], password)
        assert verified.returncode == 0 and verified.stdout == b"Password verified\n" and not verified.stderr
        for page in ["0", str(len(rows) + 1)]:
            failed = run([args.reader, path, "--page", page], password)
            assert failed.returncode == 2 and not failed.stdout, (name, "page refusal")
        assert hashlib.sha256(path.read_bytes()).hexdigest() == before
        print(f"{name}: {len(rows)} decoded page hashes agree; credential/page refusals pass; unchanged {before}")
    for name in ["db2007-oldenc.accdb", "db-nonstandard.accdb"]:
        path = args.directory / name
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        assert before == CASES[name][0]
        failed = run([args.reader, path], "Test123")
        assert failed.returncode == 2 and not failed.stdout and b"encryption profile is not supported" in failed.stderr
        assert hashlib.sha256(path.read_bytes()).hexdigest() == before
        print(f"{name}: non-Agile codec refused; unchanged {before}")


if __name__ == "__main__":
    main()
