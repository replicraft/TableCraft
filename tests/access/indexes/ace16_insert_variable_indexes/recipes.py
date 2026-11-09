"""Closed variable-key source requests, independent of any producer output."""

FAMILIES = ('Binary', 'Legacy', 'Modern')
SHAPES = {'Single': (2,), 'PayloadThenGroup': (2, 1), 'GroupThenPayload': (1, 2)}


def make(profile, family, shape, directions, mode, policy='ordinary'):
    case = dict(profile=profile, family=family, shape=shape, directions=directions,
                mode=mode, policy=policy)
    case['id'] = '-'.join(case.values())
    case['native_outcome'] = 'refusal' if mode == 'digest-unique-guard' else 'image'
    return case


def cases():
    result = []
    for profile in ('V2016', 'V2019'):
        for family in FAMILIES:
            for direction in ('A', 'D'):
                result.append(make(profile, family, 'Single', direction, 'fitted'))
            for shape in ('PayloadThenGroup', 'GroupThenPayload'):
                result.append(make(profile, family, shape, 'AD', 'fitted'))
            result.append(make(profile, family, 'PayloadThenGroup', 'DA', 'fitted', 'unique-ignored'))
        for direction in ('A', 'D'):
            result.append(make(profile, 'Binary', 'Single', direction, 'pressure'))
            result.append(make(profile, 'Modern', 'Single', direction, 'digest'))
            result.append(make(profile, 'Modern', 'Single', direction, 'digest-unique-guard', 'unique'))
    assert len(result) == len({case['id'] for case in result}) == 42
    assert sum(case['native_outcome'] == 'image' for case in result) == 38
    return result


def validate(case):
    assert any(all(case.get(key) == value for key, value in closed.items()) for closed in cases())


def source_rows(case):
    validate(case)
    if case['mode'] == 'pressure':
        return [(position, None, bytes([position]) * 255) for position in range(1, 13)] + [(13, None, None)]
    if case['mode'].startswith('digest'):
        return [(1, None, b'a' * 255)]
    if case['policy'] == 'unique-ignored':
        return [(1, None, None), (2, None, b'a'), (3, 10, None),
                (4, None, b''), (5, 20, b'aaa')]
    return [(1, None, None), (2, 10, b''), (3, None, b'a'),
            (4, 20, b'aaa'), (5, None, b'z')]


def requests(case):
    validate(case)
    if case['mode'] == 'pressure':
        return [(14, None, bytes([255]) * 255), (15, None, bytes([254]) * 255),
                (16, None, bytes([253]) * 255)]
    if case['mode'] == 'digest':
        return [(10, None, b'a' * 255), (11, None, b'z' * 255)]
    if case['mode'] == 'digest-unique-guard':
        return [(10, None, b'aa')]
    if case['policy'] == 'unique-ignored':
        return [(10, None, None), (11, 11, None), (12, None, b'aa'),
                (13, 11, b''), (14, None, None)]
    maximum = bytes([255]) * 255 if case['family'] == 'Binary' else b'z' * 255
    return [(10, None, b'aa'), (11, 20, None), (12, None, None),
            (13, None, b''), (14, 30, maximum)]


def eligible(row, case):
    return not (case['policy'] == 'unique-ignored'
                and all(row[column] is None for column in SHAPES[case['shape']]))


def binary_component(value, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) <= 255
    result = bytearray([128 if descending else 127])
    for position in range(0, len(value), 8):
        chunk = value[position:position + 8]
        final = position + 8 >= len(value)
        result.extend(bytes(byte ^ (255 if descending else 0) for byte in chunk))
        result.extend(bytes([255 if descending else 0]) * (8 - len(chunk)))
        result.append((len(chunk) ^ (255 if descending else 0)) if final else 9)
    return bytes(result)


def text_component(value, family, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) <= 255
    weights = {97: bytes.fromhex('4a'), 122: bytes.fromhex('78')} if family == 'Legacy' else {
        97: bytes.fromhex('0e02'), 122: bytes.fromhex('0ea9')}
    assert all(byte in weights for byte in value)
    payload = b''.join(weights[byte] for byte in value) + bytes([1, 0])
    if descending:
        return bytes([128]) + bytes(byte ^ 255 for byte in payload) + bytes([0])
    return bytes([127]) + payload


def encoded_key(row, case, primary=False):
    from ace16_insert_guid_tuple_indexes.recipes import integer_component
    from variable_index_split_originals.source_weights import truncate_key

    if primary:
        return integer_component(row[0], False)
    result = bytearray()
    for number, direction in zip(SHAPES[case['shape']], case['directions'], strict=True):
        descending = direction == 'D'
        if number == 1:
            result.extend(integer_component(row[number], descending))
        elif case['family'] == 'Binary':
            result.extend(binary_component(row[number], descending))
        else:
            result.extend(text_component(row[number], case['family'], descending))
    return truncate_key(bytes(result))
