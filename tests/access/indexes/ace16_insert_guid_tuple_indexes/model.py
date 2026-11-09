"""Complete SOURCE+REQUEST engineering images for the closed GUID/Int32 tuple corpus.

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


SHAPES = {'Guid': (1,), 'GuidThenScore': (1, 2), 'ScoreThenGuid': (2, 1)}


def validate_case(case):
    assert case['profile'] in ('V2016', 'V2019')
    assert case['shape'] in SHAPES
    assert case['mode'] in ('pressure', 'fitted')
    assert case['policy'] in ('ordinary', 'unique', 'unique-ignored')
    assert len(case['directions']) == len(SHAPES[case['shape']])
    assert set(case['directions']) <= {'A', 'D'}
    assert case['id'] == '-'.join(case[name] for name in (
        'profile', 'shape', 'directions', 'mode', 'policy'))
    if case['mode'] == 'pressure':
        assert case['policy'] == 'ordinary'
    else:
        assert case['shape'] != 'Guid'


def guid_value(seed):
    assert type(seed) is int and 0 <= seed < 1 << 31
    return seed.to_bytes(4, 'big') + bytes.fromhex('0405060780090a0b0c0d0eff')


def requests(case):
    validate_case(case)
    if case['mode'] == 'pressure':
        count = 157 if case['shape'] == 'Guid' else 128
        return [(count + 2, guid_value(count), count - 64)]
    if case['policy'] == 'ordinary':
        return [(7, guid_value(0), 10), (8, None, 30),
                (9, guid_value(2), None), (10, None, None), (11, None, None)]
    return [(7, guid_value(4), 40), (8, None, 31),
            (9, guid_value(3), None), (10, None, None), (11, None, None)]


def eligible(row, case):
    return not (case['policy'] == 'unique-ignored'
                and all(row[column] is None for column in SHAPES[case['shape']]))


def guid_component(value, descending):
    if value is None:
        return bytes([255 if descending else 0])
    assert type(value) is bytes and len(value) == 16
    data = bytes(byte ^ 255 for byte in value) if descending else value
    # Both tags are physical framing; only the final length tag is inverted.
    return bytes([128 if descending else 127]) + data[:8] + bytes([9]) + data[8:] + bytes([247 if descending else 8])


def integer_component(value, descending):
    if value is None:
        result = bytes([0])
    else:
        assert type(value) is int and -(1 << 31) <= value < 1 << 31
        result = bytes([127]) + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return bytes(byte ^ 255 for byte in result) if descending else result


def key(row, physical, case):
    if physical == 0:
        return integer_component(row[0], False)
    assert physical == 1
    result = bytearray()
    for column, direction in zip(SHAPES[case['shape']], case['directions'], strict=True):
        encode = guid_component if column == 1 else integer_component
        result.extend(encode(row[column], direction == 'D'))
    return bytes(result)


def entries(rows, physical, case):
    return sorted(key(values, physical, case) + reference.to_bytes(4, 'big')
                  for reference, values in rows.items()
                  if physical == 0 or eligible(values, case))


def record(values, layout, case):
    validate_case(case)
    assert layout['columns'] == 3 and layout['variables'] == 0
    fields = layout['fields']
    assert [field['name'] for field in fields] == ['ID', 'Group', 'Score']
    assert [field['number'] for field in fields] == [0, 1, 2]
    assert values[0] is not None
    body = bytearray(layout['fixed_end'])
    body[:2] = (3).to_bytes(2, 'little')
    occupied = set(range(2))
    mask = 0
    for field, value, native, width in zip(fields, values, (4, 15, 4), (4, 16, 4), strict=True):
        assert field['native_type'] == native
        assert field['physical_width'] == field['declared_width'] == width
        assert field['flags'] == 3 and field['extended_flags'] == 0
        start = 2 + field['offset']
        positions = set(range(start, start + width))
        assert not occupied & positions and start + width <= len(body)
        occupied |= positions
        if value is None:
            continue
        if native == 15:
            assert type(value) is bytes and len(value) == 16
            payload = value[:4][::-1] + value[4:6][::-1] + value[6:8][::-1] + value[8:]
        else:
            assert type(value) is int and -(1 << 31) <= value < 1 << 31
            payload = value.to_bytes(width, 'little', signed=True)
        body[start:start + width] = payload
        mask |= 1 << field['number']
    assert max(occupied) + 1 == layout['fixed_end']
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
            assert len(value) >= 5 and (not complete or complete[-1] < value)
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
        assert result == sorted(result) and len(set(result)) == len(result)
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
