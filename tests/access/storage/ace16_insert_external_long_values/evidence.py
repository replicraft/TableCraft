"""Saved pins, finite child envelopes and committed archive verification."""
import hashlib
from pathlib import PurePosixPath
import subprocess
import tarfile


class Pins:
    def __init__(self, store):
        self.store = store
        self.items = {}
        self.occurrences = 0

    def check(self, item):
        assert self.items.setdefault(item['path'], item) == item, item['path']
        assert self.store.pin(item['path']) == item, item['path']
        self.occurrences += 1
        return self.store.path(item['path'])

    def fixed(self, path, digest):
        item = self.store.pin(path)
        assert item['sha256'] == digest, path
        return self.check(item)

    def closure(self, path):
        value = self.store.load(path)
        for item in value['inputs']:
            self.check(item)
        return value

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def report(store, pins, path, count, completed=True):
    value = store.load(path)
    assert value['completed'] is completed and value['pin_errors'] == []
    assert len(value['commands']) == count
    for item in value['inputs'] + value.get('artifacts', []):
        pins.check(item)
    for command in value['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['capture_errors'] == [] and command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
        assert command['observed_stream_bytes'] == [command['stdout']['bytes'], command['stderr']['bytes']]
        assert max(command['observed_stream_bytes']) <= 16 * 1024 * 1024
        pins.check(command['stdout'])
        pins.check(command['stderr'])
    return value


def archive(store, pins, receipt, verify_git):
    path = pins.check(receipt['source_archive'])
    manifest = store.load(pins.check(receipt['source_manifest']))
    revision = receipt['commit']
    assert receipt['exported_file_count'] == len(manifest) == 4526
    files = {}
    names = set()
    with tarfile.open(path) as stream:
        assert stream.pax_headers['comment'] == revision
        for entry in stream.getmembers():
            name = PurePosixPath(entry.name)
            assert not name.is_absolute() and '..' not in name.parts
            assert '\n' not in entry.name and '\r' not in entry.name
            assert entry.name not in names
            names.add(entry.name)
            assert entry.isdir() or entry.isfile()
            if entry.isfile():
                raw = stream.extractfile(entry).read()
                assert len(raw) == entry.size
                files[entry.name] = {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                    'git_blob': hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()}
    assert files == manifest
    result = {'verified_archive_files': len(files), 'metadata_child_processes': 0}
    if not verify_git:
        return result
    receipt_revision = 'b51c5eae8a298d36ed4ba9f5c11cd3ebdef788cc'
    receipt_pin = store.pin('docs/testing/results/ace16-insert-external-long-value-isolated-example-build-2026-10-28.json')
    pins.check(receipt_pin)
    queries = [revision + ':' + name for name in files]
    queries.append(receipt_revision + ':' + receipt_pin['path'])
    wanted = list(files.values()) + [receipt_pin]
    child = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'],
        cwd=store.path('.'), input=('\n'.join(queries) + '\n').encode(),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    assert not child.stderr and len(child.stdout) <= 128 * 1024 * 1024
    position = 0
    for expected in wanted:
        end = child.stdout.index(b'\n', position)
        header = child.stdout[position:end].split()
        assert len(header) == 3 and header[1] == b'blob'
        count = int(header[2])
        begin = end + 1
        raw = child.stdout[begin:begin + count]
        assert count == expected['bytes'] and hashlib.sha256(raw).hexdigest() == expected['sha256']
        assert child.stdout[begin + count:begin + count + 1] == b'\n'
        position = begin + count + 1
    assert position == len(child.stdout)
    return dict(result, metadata_child_processes=1, committed_source_blobs=len(files), receipt_blobs=1)
