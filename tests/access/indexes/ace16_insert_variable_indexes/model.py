"""Complete SOURCE+REQUEST engineering images for the closed finite General/Binary variable-key corpus.

No public AFTER file, candidate, Java key bytes or future Rust output is an input.
Source qualification owns metadata and raw/getter agreement; this constructor
independently frames the new records and derives every changed key and byte.
"""
from pathlib import Path
import sys

from shared_insert.derive import derive as derive_image

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'tests/access'))
from ace16_insert_multiple_indexes import model as frames

PAGE = 4096
ENTRY_START = 480
CAPACITY = 3616
MAX_SLOTS = 255
MAX_IMAGE_BYTES = 64 * 1024 * 1024


def n(raw, offset, width=4):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


SHAPES = {'Single': (2,), 'PayloadThenGroup': (2, 1), 'GroupThenPayload': (1, 2)}


def validate_case(case):
    assert case['profile'] in ('V2016', 'V2019')
    assert case['family'] in ('Binary', 'Legacy', 'Modern') and case['shape'] in SHAPES
    assert len(case['directions']) == len(SHAPES[case['shape']])
    assert set(case['directions']) <= {'A', 'D'}
    assert case['policy'] in ('ordinary', 'unique-ignored')
    assert case['native_outcome'] == 'image'  # Four guards never get an image.
    assert case['id'] == '-'.join(case[key] for key in (
        'profile', 'family', 'shape', 'directions', 'mode', 'policy'))
    if case['mode'] == 'fitted':
        if case['shape'] == 'Single':
            assert case['policy'] == 'ordinary'
        elif case['policy'] == 'ordinary':
            assert case['directions'] == 'AD'
        else:
            assert case['shape'] == 'PayloadThenGroup' and case['directions'] == 'DA'
    elif case['mode'] == 'pressure':
        assert case['family'] == 'Binary' and case['shape'] == 'Single'
        assert case['policy'] == 'ordinary'
    else:
        assert case['mode'] == 'digest' and case['family'] == 'Modern'
        assert case['shape'] == 'Single' and case['policy'] == 'ordinary'


def requests(case):
    validate_case(case)
    if case['mode'] == 'pressure':
        return [(14, None, bytes([255]) * 255), (15, None, bytes([254]) * 255),
                (16, None, bytes([253]) * 255)]
    if case['mode'] == 'digest':
        return [(10, None, b'a' * 255), (11, None, b'z' * 255)]
    if case['policy'] == 'unique-ignored':
        return [(10, None, None), (11, 11, None), (12, None, b'aa'),
                (13, 11, b''), (14, None, None)]
    maximum = bytes([255]) * 255 if case['family'] == 'Binary' else b'z' * 255
    return [(10, None, b'aa'), (11, 20, None), (12, None, None),
            (13, None, b''), (14, 30, maximum)]


def eligible(row, case):
    return not (case['policy'] == 'unique-ignored'
                and all(row[column] is None for column in SHAPES[case['shape']]))


def integer_component(value, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is int and -(1 << 31) <= value < 1 << 31
    wire = bytes([127]) + (value + (1 << 31)).to_bytes(4, 'big')
    return bytes(255 - byte for byte in wire) if descending else wire


def binary_component(value, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) <= 255
    wire = bytearray([128 if descending else 127])
    for start in range(0, len(value), 8):
        chunk = value[start:start + 8]
        continuation = start + len(chunk) < len(value)
        padded = chunk.ljust(8, bytes([0]))
        wire.extend(bytes(255 - byte for byte in padded) if descending else padded)
        wire.append(9 if continuation else (255 - len(chunk) if descending else len(chunk)))
    return bytes(wire)


def text_component(value, family, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) <= 255
    wire = bytearray([128 if descending else 127])
    for character in value:
        assert character in (ord('a'), ord('z'))
        if family == 'Legacy':
            primary = bytes([0x4a if character == ord('a') else 0x78])
        else:
            assert family == 'Modern'
            primary = bytes([0x0e, 0x02 if character == ord('a') else 0xa9])
        wire.extend(bytes(255 - byte for byte in primary) if descending else primary)
    wire.extend(bytes([254, 255, 0]) if descending else bytes([1, 0]))
    return bytes(wire)


def truncate_whole_key(wire):
    if len(wire) <= 510:
        return wire
    # Independent polynomial long division of x^8 * register modulo x^16+x^15+x^2+1.
    # This does not call the source proof's bit-serial recurrence or Java encoder.
    remainder = 0
    for byte in wire[508:-1]:
        remainder <<= 8
        for high_bit in range(23, 15, -1):
            if remainder & (1 << high_bit):
                remainder ^= 0x18005 << (high_bit - 16)
        remainder ^= byte << 8
    remainder ^= wire[-1]
    assert 0 <= remainder < 65536
    return wire[:508] + remainder.to_bytes(2, 'little')


def key(row, physical, case):
    if physical == 0:
        return integer_component(row[0], False)
    assert physical == 1
    components = []
    for number, direction in zip(SHAPES[case['shape']], case['directions'], strict=True):
        if number == 1:
            wire = integer_component(row[number], direction == 'D')
        elif case['family'] == 'Binary':
            wire = binary_component(row[number], direction == 'D')
        else:
            wire = text_component(row[number], case['family'], direction == 'D')
        components.append(wire)
    return truncate_whole_key(b''.join(components))


def entry_order(entry):
    assert len(entry) >= 5
    return entry[:-4], int.from_bytes(entry[-4:], 'big')


def entries(rows, physical, case):
    return sorted((key(values, physical, case) + reference.to_bytes(4, 'big')
                   for reference, values in rows.items()
                   if physical == 0 or eligible(values, case)), key=entry_order)


def record(values, layout, case):
    validate_case(case)
    assert layout['columns'] == 3 and layout['variables'] == 1
    fields = layout['fields']
    assert [field['name'] for field in fields] == ['ID', 'Group', 'Payload']
    assert [field['number'] for field in fields] == [0, 1, 2]
    assert values[0] is not None
    body = bytearray(layout['fixed_end'])
    body[:2] = (3).to_bytes(2, 'little')
    occupied = set(range(2))
    mask = 0
    for field, value in zip(fields[:2], values[:2], strict=True):
        assert field['native_type'] == 4 and field['physical_width'] == field['declared_width'] == 4
        assert field['flags'] == 3 and field['extended_flags'] == 0
        start = 2 + field['offset']
        span = set(range(start, start + 4))
        assert not span & occupied and start + 4 <= len(body)
        occupied |= span
        if value is not None:
            assert type(value) is int and -(1 << 31) <= value < 1 << 31
            body[start:start + 4] = value.to_bytes(4, 'little', signed=True)
            mask |= 1 << field['number']
    assert max(occupied) + 1 == layout['fixed_end']
    field = fields[2]
    binary = case['family'] == 'Binary'
    assert field['native_type'] == (9 if binary else 10)
    assert field['declared_width'] == (255 if binary else 510) and field['physical_width'] == 0
    assert field['flags'] == 2 and field['extended_flags'] == field['variable_ordinal'] == 0
    if not binary:
        assert field['sort'] == {'lcid': 1033, 'variant': 0, 'version': int(case['family'] == 'Modern')}
    start = len(body)
    value = values[2]
    if value is not None:
        assert type(value) is bytes and len(value) <= 255
        if binary:
            body.extend(value)
        else:
            assert all(character in (97, 122) for character in value)
            for character in value:
                body.extend(bytes([character, 0]))
        mask |= 4
    body.extend(len(body).to_bytes(2, 'little'))
    body.extend(start.to_bytes(2, 'little'))
    body.extend((1).to_bytes(2, 'little'))
    body.append(mask)
    return bytes(body)


def tree(raw, root, owner):
    nodes = {}
    levels = {}
    leaves = []

    def visit(number, expected_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[0] in (3, 4) and page[1] == 1 and n(page, 4) == owner
        height = page[26]
        assert height < 16 and (expected_height is None or height == expected_height)
        assert (page[0] == 4) == (height == 0)
        prefix = n(page, 24, 2)
        ends = [position for position in range(1, CAPACITY + 1)
                if page[27 + position // 8] & (1 << (position % 8))]
        assert not page[27] & 1 and not page[479] & 0xfe
        active = CAPACITY - n(page, 2, 2)
        assert (ends[-1] if ends else 0) == active
        complete = []
        children = []
        begin = 0
        for end in ends:
            payload = page[ENTRY_START + begin:ENTRY_START + end]
            if height:
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            value = payload if not complete else complete[-1][:prefix] + payload
            assert len(value) >= 5 and (not complete or entry_order(complete[-1]) < entry_order(value))
            complete.append(value)
            begin = end
        tail = n(page, 20)
        nodes[number] = {'height': height, 'entries': complete, 'tail': tail,
                         'previous': n(page, 12), 'next': n(page, 16),
                         'prefix': prefix, 'active': active}
        levels.setdefault(height, []).append(number)
        if height == 0:
            assert tail == 0
            leaves.append(number)
            return complete
        if tail:
            children.append(tail)
        assert children
        result = []
        for ordinal, child in enumerate(children):
            keys = visit(child, height - 1)
            assert keys
            if ordinal < len(complete):
                assert complete[ordinal] == keys[-1]
            result.extend(keys)
        assert result == sorted(result, key=entry_order) and len(set(result)) == len(result)
        return result

    keys = visit(root)
    for pages in levels.values():
        for ordinal, number in enumerate(pages):
            assert nodes[number]['previous'] == (pages[ordinal - 1] if ordinal else 0)
            assert nodes[number]['next'] == (pages[ordinal + 1] if ordinal + 1 < len(pages) else 0)
    return nodes, leaves, keys


def retained_leaf(original, keys, seed, owner):
    page = frames.leaf(original, keys, seed, owner)
    page[4:12] = original[4:12]
    page[20:24] = original[20:24]
    page[26] = original[26]
    return page


def derive(case, before, supplied_requests=None):
    """Bind this feature's independent codecs to the shared raw image mechanics."""
    return derive_image(
        case,
        before,
        supplied_requests,
        CAPACITY=CAPACITY,
        MAX_IMAGE_BYTES=MAX_IMAGE_BYTES,
        MAX_SLOTS=MAX_SLOTS,
        PAGE=PAGE,
        eligible=eligible,
        entries=entries,
        frames=frames,
        key=key,
        n=n,
        record=record,
        requests=requests,
        retained_leaf=retained_leaf,
        tree=tree,
    )
