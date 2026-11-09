"""Three fixed full-row batches, frozen before any original is generated."""
CASES = ('single', 'chain', 'inline')


def binary(length, salt):
    return bytes((position * 73 + salt) % 256 for position in range(length))


def rows(selected):
    if selected == 'single':
        return [(10, 10, binary(65, 10), 'Ā' * 33)]
    if selected == 'chain':
        return [(11, 11, binary(8145, 11), 'Ā' * 4073)]
    assert selected == 'inline'
    return [(identity, 10, binary(64, identity), 'Ā' * 32) for identity in range(100, 132)]


def tsv(selected):
    lines = []
    for identity, group, blob, memo in rows(selected):
        fields = ['int32', str(identity), 'int32', str(group), 'binary', blob.hex(), 'text', memo]
        assert max(len(value.encode('utf-8')) for value in fields) <= 16384
        lines.append('\t'.join(fields) + '\n')
    result = ''.join(lines).encode('utf-8')
    assert len(result) <= 16777216
    return result
