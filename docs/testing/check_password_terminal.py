"""Exercise hidden CLI password input on a Unix pseudo-terminal, using public test credentials."""
import argparse
import errno
import hashlib
import os
from pathlib import Path
import pty
import select
import signal
import termios
import time
import tempfile
import subprocess


def wait_hidden(terminal):
    # The console library prints the prompt before applying input mode. Wait for
    # actual input readiness rather than racing that independent system call.
    deadline = time.monotonic() + 1
    while termios.tcgetattr(terminal)[3] & termios.ECHO:
        assert time.monotonic() < deadline, "password echo remained enabled"
        time.sleep(0.001)


def check(cli, source, password, succeeds):
    pid, terminal = pty.fork()
    if pid == 0:
        os.execl(str(cli), str(cli), "--format", "tsv", "tables", str(source), "--count")
    data = bytearray()
    answered = False
    reaped = False
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if not select.select([terminal], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(terminal, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            data.extend(chunk)
            if not answered and b"Database password: " in data:
                wait_hidden(terminal)
                os.write(terminal, password.encode() + b"\n")
                answered = True
        else:
            raise AssertionError("terminal password interaction timed out")
        _, status = os.waitpid(pid, 0)
        reaped = True
        assert answered, "password prompt absent"
        assert password.encode() not in data, "password appeared in terminal output"
        assert termios.tcgetattr(terminal)[3] & termios.ECHO, "terminal echo not restored"
        assert os.waitstatus_to_exitcode(status) == (0 if succeeds else 2), data
        assert (b"Table1\tUser\t1" in data) == succeeds, data
        if not succeeds:
            assert b"incorrect database password" in data, data
        assert data.count(b"Database password: ") == 1, "unexpected retry"
    finally:
        if not reaped:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        os.close(terminal)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    source = (args.directory / "db2007-enc.accdb").resolve()
    before = hashlib.sha256(source.read_bytes()).hexdigest()
    assert before == "fa6cd4ed7639dee40ad288ff6c530356ea050bde36dbde5e4328fee6aa07f306"
    check(args.cli.resolve(), source, "Test123", True)
    check(args.cli.resolve(), source, "public-wrong-password", False)
    with tempfile.TemporaryDirectory(prefix="tablecraft-password-terminal-") as directory:
        for wrong_confirmation in [False, True]:
            output = Path(directory) / f"changed-{wrong_confirmation}.accdb"
            rewrite(args.cli.resolve(), source, output, wrong_confirmation)
        plain = Path(directory) / "plain.accdb"
        removed = subprocess.run([str(args.cli.resolve()), "encryption", "remove", str(source), "--output", str(plain), "-p", "Test123"], capture_output=True)
        assert removed.returncode == 0, removed.stderr
        for wrong_confirmation in [False, True]:
            output = Path(directory) / f"added-{wrong_confirmation}.accdb"
            rewrite(args.cli.resolve(), plain, output, wrong_confirmation, action="add")
    assert hashlib.sha256(source.read_bytes()).hexdigest() == before
    print("Terminal password success/failure: hidden input, restored echo, no retries, unchanged source")


def rewrite(cli, source, output, wrong_confirmation, action="change-password"):
    answers = [(b"Database password: ", "Test123"),
               (b"New database password: ", "new public password"),
               (b"Confirm new database password: ", "wrong confirmation" if wrong_confirmation else "new public password")]
    if action == "add":
        answers = answers[1:]
    pid, terminal = pty.fork()
    if pid == 0:
        os.execl(str(cli), str(cli), "encryption", action, str(source), "--output", str(output))
    data = bytearray()
    answered = 0
    reaped = False
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if not select.select([terminal], [], [], 0.1)[0]:
                continue
            try:
                chunk = os.read(terminal, 4096)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if not chunk:
                break
            data.extend(chunk)
            if answered < len(answers) and answers[answered][0] in data:
                wait_hidden(terminal)
                os.write(terminal, answers[answered][1].encode() + b"\n")
                answered += 1
        else:
            raise AssertionError("password rewrite prompt timed out")
        _, status = os.waitpid(pid, 0)
        reaped = True
        assert answered == len(answers)
        assert termios.tcgetattr(terminal)[3] & termios.ECHO
        assert os.waitstatus_to_exitcode(status) == (2 if wrong_confirmation else 0), data
        for _, password in answers:
            assert password.encode() not in data
        assert output.exists() != wrong_confirmation
        if not wrong_confirmation:
            opened = subprocess.run([str(cli), "--format", "tsv", "tables", str(output), "--count", "-p", "new public password"], capture_output=True)
            assert opened.returncode == 0 and b"Table1\tUser\t1" in opened.stdout
    finally:
        if not reaped:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        os.close(terminal)


if __name__ == "__main__":
    main()
