"""Complete SOURCE+REQUEST engineering images for the closed GUID/Int32 tuple corpus.

No public AFTER file, candidate, Java key bytes or future Rust output is an input.
Source qualification owns metadata and raw/getter agreement; this constructor
independently frames the new records and derives every changed key and byte.
"""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
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
    requested = requests(case)
    if supplied_requests is not None:
        assert list(supplied_requests) == requested
    raw = bytearray(before['raw'])
    assert 0 < len(raw) <= MAX_IMAGE_BYTES and len(raw) % PAGE == 0
    definition = before['table']['definition']
    rows = dict(before['rows'])
    hints = set(before['maps']['hint']['members'])
    new_data = []
    placements = []
    for values in requested:
        body = record(values, before['layout'], case)
        destination = max(hints) if hints else None
        if destination is not None:
            base = destination * PAGE
            if n(raw, base + 12, 2) >= MAX_SLOTS or n(raw, base + 2, 2) < len(body) + 2:
                hints.remove(destination)
                destination = None
        if destination is None:
            destination = len(raw) // PAGE
            raw.extend(bytes(PAGE))
            raw[destination * PAGE:destination * PAGE + 2] = bytes([1, 1])
            frames.put(raw, destination * PAGE + 4, definition)
            frames.put(raw, destination * PAGE + 2, PAGE - 14, 2)
            hints.add(destination)
            new_data.append(destination)
        base = destination * PAGE
        assert raw[base:base + 2] == bytes([1, 1]) and n(raw, base + 4) == definition
        slot = n(raw, base + 12, 2)
        assert slot < MAX_SLOTS
        end = PAGE if slot == 0 else n(raw, base + 12 + 2 * slot, 2)
        begin = end - len(body)
        assert begin >= 14 + 2 * (slot + 1)
        raw[base + begin:base + end] = body
        frames.put(raw, base + 14 + 2 * slot, begin, 2)
        frames.put(raw, base + 12, slot + 1, 2)
        frames.put(raw, base + 2, begin - 14 - 2 * (slot + 1), 2)
        reference = destination * 256 + slot
        assert reference not in rows
        rows[reference] = values
        placements.append({'id': values[0], 'ROW': reference, 'page': destination,
                           'slot': slot, 'record_hex': body.hex()})

    members = []
    allocations = []
    assert len(before['physical']) == len(before['trees']) == 2
    for physical, metadata in enumerate(before['physical']):
        root = metadata['root']
        original = before['raw'][root * PAGE:(root + 1) * PAGE]
        nodes, leaves, old_keys = tree(before['raw'], root, definition)
        assert set(nodes) == {root} and original[0] == 4 and original[26] == 0
        assert nodes[root]['previous'] == nodes[root]['next'] == nodes[root]['tail'] == 0
        assert old_keys == entries(before['rows'], physical, case)
        final_keys = entries(rows, physical, case)
        if final_keys == old_keys:
            members.append({root})
            continue
        seed = n(original, 24, 2)
        if frames.framing(final_keys, seed)[1] <= CAPACITY:
            raw[root * PAGE:(root + 1) * PAGE] = retained_leaf(original, final_keys, seed, definition)
            members.append({root})
        else:
            middle = len(final_keys) // 2
            assert frames.framing(final_keys[:middle], 0)[1] <= CAPACITY
            assert frames.framing(final_keys[middle:], 0)[1] <= CAPACITY
            right = len(raw) // PAGE
            left = right + 1
            raw.extend(bytes(2 * PAGE))
            raw[right * PAGE:(right + 1) * PAGE] = frames.leaf(
                None, final_keys[middle:], 0, definition, left, 0)
            raw[left * PAGE:(left + 1) * PAGE] = frames.leaf(
                None, final_keys[:middle], 0, definition, 0, right)
            grown = frames.root(original, final_keys[middle - 1], left, right, definition)
            grown[4:20] = original[4:20]
            raw[root * PAGE:(root + 1) * PAGE] = grown
            members.append({root, right, left})
            allocations.extend([{'physical': physical, 'page': right, 'purpose': 'INDEX'},
                                {'physical': physical, 'page': left, 'purpose': 'INDEX'}])

    original_eof = len(before['raw']) // PAGE
    suffix = set(range(original_eof, len(raw) // PAGE))
    new_indexes = {item['page'] for item in allocations}
    assert suffix == set(new_data) | new_indexes and not set(new_data) & new_indexes
    assert suffix <= before['maps']['global']['members']
    wanted_maps = {
        'owned': before['maps']['owned']['members'] | set(new_data),
        'hint': hints,
        'global': before['maps']['global']['members'] - suffix,
        'index0': members[0],
        'index1': members[1],
    }
    map_changes = []
    for role, wanted in wanted_maps.items():
        metadata = before['maps'][role]
        for member in sorted(metadata['members'] ^ wanted):
            assert member in metadata['locations']
            offset, mask = metadata['locations'][member]
            old = raw[offset]
            raw[offset] ^= mask
            map_changes.append({'role': role, 'member': member, 'offset': offset,
                                'mask': mask, 'before': old, 'after': raw[offset]})
    frames.put(raw, definition * PAGE + 16, len(rows))
    original_families = {key(value, 1, case) for value in before['rows'].values()
                         if eligible(value, case)}
    added_families = {key(value, 1, case) for value in requested if eligible(value, case)}
    deltas = [len(requested), len(added_families - original_families)]
    for metadata, delta in zip(before['physical'], deltas, strict=True):
        original = n(before['raw'], metadata['history'])
        assert original + delta < 1 << 32
        frames.put(raw, metadata['history'], original + delta)

    result = bytes(raw)
    assert len(result) <= MAX_IMAGE_BYTES
    for reference in before['rows']:
        page = reference >> 8
        slot = reference & 255
        end = PAGE if slot == 0 else n(before['raw'], page * PAGE + 12 + 2 * slot, 2)
        begin = n(before['raw'], page * PAGE + 14 + 2 * slot, 2)
        assert result[page * PAGE + begin:page * PAGE + end] == before['raw'][page * PAGE + begin:page * PAGE + end]
    trees = []
    for physical, metadata in enumerate(before['physical']):
        nodes, leaves, keys = tree(result, metadata['root'], definition)
        assert set(nodes) == members[physical] and keys == entries(rows, physical, case)
        trees.append({'physical': physical, 'root': metadata['root'],
                      'height': nodes[metadata['root']]['height'], 'leaves': leaves,
                      'entries': len(keys)})
    return result, {'placements': placements, 'new_data': new_data,
                    'allocations': allocations, 'counter_deltas': deltas,
                    'rows': len(rows), 'eligible_secondary': sum(eligible(row, case) for row in rows.values()),
                    'trees': trees, 'map_changes': map_changes,
                    'image_policy': 'Complete source/request-only engineering bytes; no AFTER input.'}
