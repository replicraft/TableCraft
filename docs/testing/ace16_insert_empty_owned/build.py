"""Verify the retained committed source archive, with optional Git metadata only."""
import hashlib
from pathlib import PurePosixPath
import subprocess
import tarfile

RECEIPT = 'docs/testing/results/ace16-insert-empty-owned-example-build-2026-10-28.json'
REVISION = '463853e1e2a7dae00bb549538b7a049e21d376cc'


def verify(store, pins, verify_git):
    receipt = store.load(RECEIPT)
    assert receipt['commit'] == REVISION and receipt['status'] == 0
    assert receipt['fresh_target_was_absent'] and receipt['owning_engine_compilation_observed']
    assert receipt['source_inventory_unchanged_after_build']
    for key in ('artifact', 'build_log', 'build_driver', 'build_freeze'):
        pins.check(receipt[key])
    assert 'Compiling tablecraft-ace ' in pins.check(receipt['build_log']).read_text()
    archive_path = pins.check(receipt['source_archive'])
    manifest = store.load(pins.check(receipt['source_manifest']))
    assert receipt['exported_file_count'] == len(manifest) == 4620
    files = {}
    names = set()
    with tarfile.open(archive_path) as archive:
        assert archive.pax_headers['comment'] == REVISION
        for entry in archive.getmembers():
            name = PurePosixPath(entry.name)
            assert not name.is_absolute() and '..' not in name.parts
            assert '\n' not in entry.name and '\r' not in entry.name
            assert entry.name not in names
            names.add(entry.name)
            assert entry.isdir() or entry.isfile()
            if entry.isfile():
                raw = archive.extractfile(entry).read()
                assert len(raw) == entry.size
                files[entry.name] = {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                    'git_blob': hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()}
    assert files == manifest
    result = {'archive_files': len(files), 'metadata_child_processes': 0}
    if not verify_git:
        return receipt, result
    receipt_revision = 'd9e9f40d'
    # This exact historical receipt commit is checked along with every archive
    # blob; no mutable working-tree source substitutes for the build inputs.
    receipt_pin = store.pin(RECEIPT)
    queries = [REVISION + ':' + name for name in files] + [receipt_revision + ':' + RECEIPT]
    expected = list(files.values()) + [receipt_pin]
    child = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'],
        cwd=store.path('.'), input=('\n'.join(queries) + '\n').encode(),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    assert not child.stderr and len(child.stdout) <= 128 * 1024 * 1024
    position = 0
    for item in expected:
        end = child.stdout.index(b'\n', position)
        header = child.stdout[position:end].split()
        assert len(header) == 3 and header[1] == b'blob'
        count = int(header[2])
        begin = end + 1
        assert count == item['bytes']
        assert hashlib.sha256(child.stdout[begin:begin + count]).hexdigest() == item['sha256']
        assert child.stdout[begin + count:begin + count + 1] == b'\n'
        position = begin + count + 1
    assert position == len(child.stdout)
    return receipt, dict(result, metadata_child_processes=1,
                         committed_source_blobs=len(files), receipt_blobs=1)
