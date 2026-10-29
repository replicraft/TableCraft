"""Bound fixture processes and retain captured prefixes on every failure."""
import hashlib
import json
from pathlib import Path
import subprocess
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
TIMEOUT_SECONDS = 30
MAX_STREAM_BYTES = 16 * 1024 * 1024
MAX_PROCESSES = 1
CHUNK_BYTES = 65536
POLL_SECONDS = 0.02


def path(value):
    result = (ROOT / value).resolve()
    result.relative_to(ROOT)
    return result


def pin(value):
    target = path(value)
    data = target.read_bytes()
    return {'path': target.relative_to(ROOT).as_posix(), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def require_pin(expected):
    assert pin(expected['path']) == expected, expected['path']


class Capture:
    def __init__(self, work):
        self.work = work
        self.records = []

    def run(self, argv, label, timeout_seconds=TIMEOUT_SECONDS):
        assert timeout_seconds == TIMEOUT_SECONDS
        assert len(self.records) < MAX_PROCESSES
        stdout = self.work / (label + '.stdout')
        stderr = self.work / (label + '.stderr')
        observed = [0, 0]
        errors = []
        over_limit = threading.Event()
        process = None
        readers = []
        stopped_for = None
        status = None

        def drain(source, destination, position):
            try:
                with destination.open('xb') as output:
                    while True:
                        chunk = source.read(CHUNK_BYTES)
                        if not chunk:
                            break
                        remaining = max(0, MAX_STREAM_BYTES - observed[position])
                        output.write(chunk[:remaining])
                        observed[position] += len(chunk)
                        if observed[position] > MAX_STREAM_BYTES:
                            over_limit.set()
            except BaseException as error:
                errors.append(type(error).__name__)
                over_limit.set()
            finally:
                source.close()

        print('Starting fixture process:', label, flush=True)
        try:
            process = subprocess.Popen(argv, cwd=ROOT, stdout=subprocess.PIPE,
                                       stderr=subprocess.PIPE)
            for position, (source, destination) in enumerate([
                (process.stdout, stdout), (process.stderr, stderr),
            ]):
                thread = threading.Thread(target=drain, args=(source, destination, position),
                                          daemon=True)
                thread.start()
                readers.append(thread)
            deadline = time.monotonic() + timeout_seconds
            while process.poll() is None:
                if over_limit.is_set():
                    stopped_for = 'stream ceiling or capture error'
                elif time.monotonic() >= deadline:
                    stopped_for = 'timeout'
                if stopped_for:
                    process.kill()
                    break
                time.sleep(POLL_SECONDS)
            try:
                status = process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                stopped_for = 'killed process did not reap within two seconds'
            for thread in readers:
                thread.join(timeout=2)
            if any(thread.is_alive() for thread in readers):
                stopped_for = 'capture pipe did not close within two seconds'
            elif over_limit.is_set():
                stopped_for = 'stream ceiling or capture error'
        except BaseException as error:
            stopped_for = type(error).__name__
            if process is not None:
                if process.poll() is None:
                    process.kill()
                try:
                    status = process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    stopped_for += '; child did not reap within two seconds'
            for thread in readers:
                thread.join(timeout=2)
            if any(thread.is_alive() for thread in readers):
                stopped_for += '; capture pipe did not close within two seconds'
            raise
        finally:
            record = {'label': label, 'argv': argv, 'status': status,
                      'stopped_for': stopped_for, 'timeout_seconds': timeout_seconds,
                      'stream_limit_bytes': MAX_STREAM_BYTES, 'observed_stream_bytes': observed,
                      'capture_errors': errors,
                      'stdout': pin(stdout) if stdout.exists() else None,
                      'stderr': pin(stderr) if stderr.exists() else None}
            self.records.append(record)
            with (self.work / 'commands.jsonl').open('a') as output:
                output.write(json.dumps(record) + '\n')
            print('Completed fixture process:', label, 'status', status, flush=True)
        assert status == 0 and stopped_for is None, record
        return stdout
