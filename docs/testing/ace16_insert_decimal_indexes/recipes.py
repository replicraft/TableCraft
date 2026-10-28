"""Closed Decimal recipes; coefficients never inherit another field's scale."""
from itertools import product

SHAPES = {
    'Decimal': (1,),
    'DecimalThenGuid': (1, 2),
    'GuidThenDecimal': (2, 1),
    'DecimalThenDecimal': (1, 2),
    'ReversedDecimals': (2, 1),
}
DOMAINS = ((1, 0), (3, 2), (28, 4), (28, 28))


def cases():
    result = []
    for profile in ('V2016', 'V2019'):
        for precision, scale in DOMAINS:
            for direction in ('A', 'D'):
                result.append(make(profile, 'Decimal', direction, 'fitted', 'ordinary', precision, scale))
        for direction in ('A', 'D'):
            result.append(make(profile, 'Decimal', direction, 'pressure', 'ordinary', 28, 4))
        for shape, direction in product(('DecimalThenGuid', 'GuidThenDecimal'), ('AA', 'AD', 'DA', 'DD')):
            result.append(make(profile, shape, direction, 'pressure', 'ordinary', 28, 4))
        for shape, policy in product(('DecimalThenDecimal', 'ReversedDecimals'), ('ordinary', 'unique', 'unique-ignored')):
            result.append(make(profile, shape, 'AD', 'fitted', policy, 28, 4))
    assert len(result) == len({case['id'] for case in result}) == 48
    return result


def make(profile, shape, directions, mode, policy, precision, scale):
    case = dict(profile=profile, shape=shape, directions=directions, mode=mode,
                policy=policy, precision=precision, scale=scale)
    case['id'] = '-'.join((profile, shape, directions, mode, policy, f'p{precision}s{scale}'))
    return case


def domains(case):
    return {1: (case['precision'], case['scale']),
            **({2: (3, 2)} if case['shape'] in ('DecimalThenDecimal', 'ReversedDecimals') else {})}


def guid(seed):
    assert 0 <= seed < 2 ** 31
    return seed.to_bytes(4, 'big') + bytes.fromhex('0405060780090a0b0c0d0eff')


def source_rows(case):
    if case['mode'] == 'pressure':
        count = 164 if case['shape'] == 'Decimal' else 88
        return [(position + 1, position - count // 2,
                 None if case['shape'] == 'Decimal' else guid(position))
                for position in range(count)] + [(count + 1, None, None)]
    limit = 10 ** case['precision'] - 1
    if case['shape'] == 'Decimal':
        return [(position + 1, coefficient, None)
                for position, coefficient in enumerate((-limit, -1, 0, 1, limit, None))]
    return [(1, limit, -999), (2, limit, 999), (3, None, 0),
            (4, 0, None), (5, None, None), (6, -limit, -1)]


def requests(case):
    if case['mode'] == 'pressure':
        count = 164 if case['shape'] == 'Decimal' else 88
        return [(count + 2, count, None if case['shape'] == 'Decimal' else guid(count))]
    if case['shape'] == 'Decimal':
        limit = 10 ** case['precision'] - 1
        return [(7, -limit, None), (8, limit, None), (9, 0, None),
                (10, None, None), (11, None, None)]
    if case['policy'] == 'ordinary':
        limit = 10 ** case['precision'] - 1
        return [(7, limit, -999), (8, None, 0), (9, 0, None),
                (10, None, None), (11, None, None)]
    return [(7, 1, 1), (8, None, 1), (9, -1, None),
            (10, None, None), (11, None, None)]


def eligible(row, case):
    return not (case['policy'] == 'unique-ignored'
                and all(row[column] is None for column in SHAPES[case['shape']]))


def decimal_component(coefficient, descending):
    if coefficient is None:
        return bytes([255 if descending else 0])
    assert type(coefficient) is int and abs(coefficient) < 10 ** 28
    magnitude = abs(coefficient).to_bytes(16, 'big')
    data = bytes([0 if coefficient < 0 else 255]) + bytes(
        byte ^ (255 if coefficient < 0 else 0) for byte in magnitude)
    return bytes([128 if descending else 127]) + bytes(
        byte ^ (255 if descending else 0) for byte in data)


def decimal_field(coefficient, precision, scale):
    assert 1 <= precision <= 28 and 0 <= scale <= precision
    if coefficient is None:
        return b''
    assert type(coefficient) is int and abs(coefficient) < 10 ** precision
    magnitude = abs(coefficient).to_bytes(16, 'big')
    return bytes([128 if coefficient < 0 else 0]) + b''.join(
        magnitude[offset:offset + 4][::-1] for offset in range(0, 16, 4))


def validate(case):
    assert any(all(case.get(key) == value for key, value in closed.items())
               for closed in cases())


def encoded_key(row, case, primary=False):
    from ace16_insert_guid_tuple_indexes.recipes import guid_component, integer_component

    if primary:
        return integer_component(row[0], False)
    result = b''
    for column, direction in zip(SHAPES[case['shape']], case['directions'], strict=True):
        value = row[column]
        if column in domains(case):
            result += decimal_component(value, direction == 'D')
        else:
            result += guid_component(value, direction == 'D')
    return result
