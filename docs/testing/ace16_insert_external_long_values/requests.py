"""Immutable full-row requests; no Java getter or producer imports."""
from pathlib import Path

CASES = ('single', 'chain', 'carriers', 'mixed')


def binary(length, salt):
    return bytes((position * 73 + salt) % 256 for position in range(length))


def rows(case):
    values = {
        'single': [(10, 10, binary(65, 10), 'Ā' * 33),
                   (11, 10, binary(4076, 11), 'Ā' * 2038)],
        'chain': [(12, 11, binary(4077, 12), 'Ā' * 2039)],
        'carriers': [(20, None, None, None), (21, 7, b'', ''),
                     (22, 9, binary(64, 22), 'ABC')],
        'mixed': [(30, 10, binary(4077, 30), 'ABC'),
                  (31, 11, binary(65, 31), 'Ā' * 2039)],
    }
    assert case in CASES
    return values[case]


def tsv(case):
    encoded = []
    for identity, group, blob, memo in rows(case):
        fields = ['int32', str(identity)]
        fields += ['null', ''] if group is None else ['int32', str(group)]
        fields += ['null', ''] if blob is None else ['binary', blob.hex()]
        fields += ['null', ''] if memo is None else ['text', memo]
        assert all('\t' not in field and '\n' not in field for field in fields)
        encoded.append('\t'.join(fields) + '\n')
    result = ''.join(encoded).encode('utf-8')
    assert len(result) < 16 * 1024 * 1024
    assert max(len(field) for line in result.splitlines() for field in line.split(b'\t')) <= 16384
    return result


def describe(case):
    return [dict(identity=identity, group=group,
                 blob=None if blob is None else blob.hex(),
                 memo=None if memo is None else memo.encode('utf-16le').hex())
            for identity, group, blob, memo in rows(case)]


def verify_files(directory):
    directory = Path(directory)
    for case in CASES:
        assert (directory / (case + '.tsv')).read_bytes() == tsv(case)
