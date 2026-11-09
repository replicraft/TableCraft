"""Bounded optional Git-object protocol verification, with no automatic calls."""
import hashlib
import subprocess

from engineering_paths import repository_root

ROOT = repository_root()


def git_objects(binding):
    pins = [binding['build_receipt']] + binding['compiled_sources']
    queries = [binding['receipt_revision'] + ':' + pins[0]['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in pins[1:]]
    assert all('\n' not in item and '\r' not in item for item in queries)
    assert sum(item['bytes'] for item in pins) <= 64 * 1024 * 1024
    result = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    position = 0
    for item in pins:
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = stop + 1
        data = result.stdout[begin:begin + size]
        assert size == item['bytes'] and hashlib.sha256(data).hexdigest() == item['sha256']
        assert result.stdout[begin + size:begin + size + 1] == b'\n'
        position = begin + size + 1
    assert position == len(result.stdout)
    return {'verified': True, 'compiled_sources': len(binding['compiled_sources']),
            'receipt_objects': 1, 'metadata_processes': 1}
