"""Explicit repository paths, inert Java commands and bounded evidence capture."""
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
PRODUCER_SHA256 = '2d92ce90bd970c0d613ddc8a6bca3146a551c48f5aef3026c728dd69822493a6'
TIMEOUT_SECONDS = 30
POLL_SECONDS = 0.05


def path(value):
    result = (ROOT / value).resolve()
    result.relative_to(ROOT)
    return result


def pin(value):
    target = path(value)
    data = target.read_bytes()
    return {'path': str(target.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def safe_label(value):
    allowed = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_'
    if not value or any(character not in allowed for character in value):
        raise ValueError('Evidence labels must be simple ASCII names, not paths')
    return value


def java(jar, classes):
    assert pin(jar)['sha256'] == JAR_SHA256
    return ['java', '-Dstdout.encoding=UTF-8', '-cp',
            os.pathsep.join([str(path(jar)), str(path(classes))]), 'SlotUpdateFixture']


def fresh_work(value):
    result = path(value)
    result.relative_to(ROOT / '.cache')
    assert not result.exists(), 'Use a fresh work directory'
    result.mkdir(parents=True)
    return result


class Capture:
    def __init__(self, directory):
        self.directory = path(directory)
        self.commands = []

    def run(self, argv, stem):
        out = self.directory / (stem + '.stdout')
        err = self.directory / (stem + '.stderr')
        with out.open('xb') as stdout, err.open('xb') as stderr:
            process = subprocess.Popen(argv, cwd=ROOT, stdout=stdout, stderr=stderr)
            deadline = time.monotonic() + TIMEOUT_SECONDS
            status = process.poll()
            while status is None and time.monotonic() < deadline:
                time.sleep(POLL_SECONDS)
                status = process.poll()
            timed_out = status is None
            if timed_out:
                try:
                    process.kill()
                except ProcessLookupError:
                    status = process.poll()
                # Never block waiting for an uninterruptible aborted child.
        record = {'argv': argv, 'status': status, 'timed_out': timed_out,
                  'timeout_seconds': TIMEOUT_SECONDS, 'stdout': pin(out), 'stderr': pin(err)}
        self.commands.append(record)
        with (self.directory / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(record) + '\n')
        return record
