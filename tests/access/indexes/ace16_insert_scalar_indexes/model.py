"""Complete source/request engineering images for the finite scalar corpus."""
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


def requests(case):
    family = case['family']
    assert case['profile'] in ('V2016', 'V2019')
    assert case['direction'] in ('ASC', 'DESC')
    assert case['id'] == '-'.join([case['profile'], family, case['direction']])
    if family == 'Boolean':
        return [(4, False, b''), (5, True, b'')]
    if family == 'Single':
        return [(402, 0x80000000, b''), (403, 0, b'')]
    if family == 'Double':
        return [(279, 0x8000000000000000, b'')]
    assert family == 'DateTime'
    return [(279, 0, b'')]


def group_key(value, case):
    family = case['family']
    if value is None:
        assert family != 'Boolean'
        result = bytearray([0])
    elif family == 'Boolean':
        assert type(value) is bool
        result = bytearray([127, 0 if value else 255])
    else:
        width = 4 if family == 'Single' else 8
        assert family in ('Single', 'Double', 'DateTime')
        assert type(value) is int and 0 <= value < 1 << (8 * width)
        sign = 1 << (8 * width - 1)
        ordered = (~value & ((1 << (8 * width)) - 1)) if value & sign else value ^ sign
        result = bytearray([127]) + ordered.to_bytes(width, 'big')
    if case['direction'] == 'DESC':
        result = bytearray(byte ^ 255 for byte in result)
    return bytes(result)


def entries(rows, physical, case):
    result = []
    for reference, values in rows.items():
        identity, group, binary = values
        assert type(identity) is int and -(1 << 31) <= identity < 1 << 31
        if physical == 0:
            key = bytes([127]) + ((identity & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
        else:
            assert physical == 1
            key = group_key(group, case)
        result.append(key + reference.to_bytes(4, 'big'))
    return sorted(result)


def record(values, layout, case):
    assert layout['columns'] == 3 and layout['variables'] == 1
    fields = layout['fields']
    assert [field['name'] for field in fields] == ['ID', 'Group', 'Bytes']
    assert [field['number'] for field in fields] == [0, 1, 2]
    assert fields[2]['variable_ordinal'] == 0
    fixed_end = layout['fixed_end']
    assert fixed_end >= 6
    body = bytearray(fixed_end)
    body[:2] = (3).to_bytes(2, 'little')
    mask = 0
    occupied = set(range(2))
    for field, value in zip(fields[:2], values[:2], strict=True):
        width = field['physical_width']
        if field['name'] == 'Group' and case['family'] == 'Boolean':
            assert width == 0 and field['declared_width'] in (0, 1)
            assert type(value) is bool
            mask |= int(value) << field['number']
            continue
        expected_width = 4 if field['name'] == 'ID' or case['family'] == 'Single' else 8
        assert width == field['declared_width'] == expected_width
        start = 2 + field['offset']
        positions = set(range(start, start + width))
        assert not occupied & positions and start + width <= fixed_end
        occupied |= positions
        if value is not None:
            assert type(value) is int
            encoded = value.to_bytes(width, 'little', signed=field['name'] == 'ID')
            body[start:start + width] = encoded
            mask |= 1 << field['number']
    assert max(occupied) + 1 == fixed_end
    assert values[2] == b''
    mask |= 1 << fields[2]['number']
    # One present empty Binary has equal start/end and a one-variable footer.
    body.extend(fixed_end.to_bytes(2, 'little'))
    body.extend(fixed_end.to_bytes(2, 'little'))
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
    original_families = {group_key(value[1], case) for value in before['rows'].values()}
    added_families = {group_key(value[1], case) for value in requested}
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
                    'rows': len(rows), 'trees': trees, 'map_changes': map_changes,
                    'image_policy': 'Complete source/request-only engineering bytes; no AFTER input.'}
