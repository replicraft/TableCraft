"""Independent shared raw insertion mechanics; callers bind their own codecs.

The algorithm consumes source bytes and requests, never an observed AFTER or a
production encoder. Domain-specific record/key/eligibility policy stays with
its feature model and is supplied explicitly at each call.
"""

def derive(
    case,
    before,
    supplied_requests=None,
    *,
    CAPACITY,
    MAX_IMAGE_BYTES,
    MAX_SLOTS,
    PAGE,
    eligible,
    entries,
    frames,
    key,
    n,
    record,
    requests,
    retained_leaf,
    tree,
):
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
