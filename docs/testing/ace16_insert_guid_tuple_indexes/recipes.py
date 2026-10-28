"""Closed GUID/Int32 recipes and independent key bytes; no source I/O."""
SHAPES = {'Guid': (1,), 'GuidThenScore': (1, 2), 'ScoreThenGuid': (2, 1)}
POLICIES = ('ordinary', 'unique', 'unique-ignored')


def validate(case):
    assert case['profile'] in ('V2016', 'V2019')
    assert case['shape'] in SHAPES and case['mode'] in ('pressure', 'fitted')
    assert case['policy'] in POLICIES
    assert len(case['directions']) == len(SHAPES[case['shape']])
    assert set(case['directions']) <= {'A', 'D'}
    if case['mode'] == 'pressure':
        assert case['policy'] == 'ordinary'
    else:
        assert case['shape'] != 'Guid'


def guid(seed):
    assert type(seed) is int and 0 <= seed < 2 ** 31
    return seed.to_bytes(4, 'big') + bytes.fromhex('0405060780090a0b0c0d0eff')


def source_rows(case):
    validate(case)
    if case['mode'] == 'pressure':
        count = 157 if case['shape'] == 'Guid' else 128
        rows = [(position + 1, guid(position), position - 64) for position in range(count)]
        score = 12345 if case['shape'] == 'Guid' else None
        return rows + [(count + 1, None, score)]
    return [(1, guid(0), 10), (2, guid(0), 20), (3, None, 30),
            (4, guid(2), None), (5, None, None), (6, bytes(16), 0)]


def requests(case):
    validate(case)
    if case['mode'] == 'pressure':
        count = 157 if case['shape'] == 'Guid' else 128
        return [(count + 2, guid(count), count - 64)]
    if case['policy'] == 'ordinary':
        return [(7, guid(0), 10), (8, None, 30), (9, guid(2), None),
                (10, None, None), (11, None, None)]
    return [(7, guid(4), 40), (8, None, 31), (9, guid(3), None),
            (10, None, None), (11, None, None)]


def eligible(row, case):
    return not (case['policy'] == 'unique-ignored'
                and all(row[column] is None for column in SHAPES[case['shape']]))


def guid_component(value, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) == 16
    data = bytes(byte ^ 255 for byte in value) if descending else value
    return bytes([128 if descending else 127]) + data[:8] + bytes([9]) + data[8:] + bytes([247 if descending else 8])


def integer_component(value, descending):
    if value is None:
        result = b'\x00'
    else:
        assert type(value) is int and -(2 ** 31) <= value < 2 ** 31
        result = b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return bytes(byte ^ 255 for byte in result) if descending else result


def encoded_key(row, case, primary=False):
    if primary:
        return integer_component(row[0], False)
    result = bytearray()
    for column, direction in zip(SHAPES[case['shape']], case['directions'], strict=True):
        encode = guid_component if column == 1 else integer_component
        result.extend(encode(row[column], direction == 'D'))
    return bytes(result)


def stored_guid(canonical):
    assert type(canonical) is bytes and len(canonical) == 16
    return canonical[:4][::-1] + canonical[4:6][::-1] + canonical[6:8][::-1] + canonical[8:]
