"""Two immutable full-row requests; independent of public AFTER or producer bytes."""
CASES = ('single', 'chain')


def binary(length, salt):
    return bytes((position * 73 + salt) % 256 for position in range(length))


def rows(selected):
    if selected == 'single':
        return [(10, 10, binary(65, 10), 'Ā' * 33)]
    assert selected == 'chain'
    return [(11, 11, binary(8145, 11), 'Ā' * 4073)]


def tsv(selected):
    lines = []
    for identity, group, blob, memo in rows(selected):
        fields = ['int32', str(identity), 'int32', str(group), 'binary', blob.hex(), 'text', memo]
        assert max(len(value.encode('utf-8')) for value in fields) <= 16384
        lines.append('\t'.join(fields) + '\n')
    return ''.join(lines).encode('utf-8')
