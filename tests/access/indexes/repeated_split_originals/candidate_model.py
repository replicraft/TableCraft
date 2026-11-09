"""Independent repeated-root split scheduling and complete-image expectation."""
import hashlib

PAGE = 4096
ENTRY = 480
BITMAP = 27
FREE = 2
OWNER = 4
PREVIOUS = 12
NEXT = 16
TAIL = 20
PREFIX = 24
HEIGHT = 26
DIRECTORY = 14
OFFSET_MASK = 0x1FFF
ROW_BYTES = 4
CHILD_BYTES = 4
CAPACITY = PAGE - ENTRY


def number(raw, offset, width=4):
    return int.from_bytes(raw[offset:offset + width], 'little')


def put(raw, offset, value, width=4):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def encode_key(units, reference, width, unsigned_byte, descending):
    if unsigned_byte:
        assert width == 1 and 0 <= units <= 255
        data = bytes([units])
    else:
        bits = width * 8
        assert -(1 << (bits - 1)) <= units < 1 << (bits - 1)
        data = ((units % (1 << bits)) ^ (1 << (bits - 1))).to_bytes(width, 'big')
    if descending:
        data = bytes(byte ^ 255 for byte in data)
    return bytes([128 if descending else 127]) + data + reference.to_bytes(ROW_BYTES, 'big')


def decode_key(entry, width, unsigned_byte, descending):
    assert len(entry) == 1 + width + ROW_BYTES
    assert entry[0] == (128 if descending else 127)
    payload = entry[1:1 + width]
    if descending:
        payload = bytes(byte ^ 255 for byte in payload)
    if unsigned_byte:
        units = payload[0]
    else:
        bits = width * 8
        units = int.from_bytes(payload, 'big') ^ (1 << (bits - 1))
        if units >= 1 << (bits - 1):
            units -= 1 << bits
    return units, int.from_bytes(entry[-ROW_BYTES:], 'big')


def common_prefix(entries):
    assert entries
    prefix = len(entries[0])
    for entry in entries[1:]:
        prefix = min(prefix, len(entry))
        for position in range(prefix):
            if entry[position] != entries[0][position]:
                prefix = position
                break
    return prefix


def layout(entries, seed):
    common = common_prefix(entries)
    prefix = min(seed, common)
    total = sum(len(entry) for entry in entries)
    saved = len(entries) - 1
    if total - prefix * saved > CAPACITY:
        prefix = common
    return prefix, total - prefix * saved


def packet(initial, entries, seed):
    result = bytearray(initial)
    result[BITMAP:ENTRY] = bytes(ENTRY - BITMAP)
    prefix, length = layout(entries, seed)
    assert length <= CAPACITY
    used = 0
    for position, entry in enumerate(entries):
        suffix = entry if position == 0 else entry[prefix:]
        assert used + len(suffix) <= CAPACITY
        result[ENTRY + used:ENTRY + used + len(suffix)] = suffix
        used += len(suffix)
        result[BITMAP + used // 8] |= 1 << (used % 8)
    assert used == length
    put(result, FREE, CAPACITY - used, 2)
    put(result, PREFIX, prefix, 2)
    return bytes(result), ENTRY + used


def fragments(entries, allocation):
    # Worklist order is physical allocation order. Logical order is separate.
    middle = len(entries) // 2
    pages = [
        {'number': allocation, 'entries': entries[middle:]},
        {'number': allocation + 1, 'entries': entries[:middle]},
    ]
    allocation += 2
    order = [1, 0]
    events = []
    passes = 0
    while True:
        passes += 1
        divided = False
        position = 0
        while position < len(pages):
            current = position
            position += 1
            page = pages[current]
            _, length = layout(page['entries'], 0)
            if length <= CAPACITY:
                continue
            middle = len(page['entries']) // 2
            assert 0 < middle < len(page['entries'])
            left = {'number': allocation, 'entries': page['entries'][:middle]}
            allocation += 1
            page['entries'] = page['entries'][middle:]
            left_index = len(pages)
            pages.append(left)
            order.insert(order.index(current), left_index)
            events.append({'pass': passes, 'right': page['number'], 'left': left['number']})
            divided = True
        if not divided:
            break
    assert len({page['number'] for page in pages}) == len(pages)
    return pages, order, allocation, events


def fresh(owner, previous, following):
    page = bytearray(PAGE)
    page[:2] = bytes([4, 1])
    put(page, OWNER, owner)
    put(page, PREVIOUS, previous)
    put(page, NEXT, following)
    return page


def decode_tree(raw, root, owner, width, unsigned_byte, descending):
    pages = {}
    leaves = []

    def visit(number_value, depth):
        assert depth < 16 and number_value not in pages
        page = raw[number_value * PAGE:(number_value + 1) * PAGE]
        assert len(page) == PAGE and page[1] == 1
        assert number(page, OWNER) == owner
        leaf = page[0] == 4
        assert page[0] in (3, 4) and leaf == (page[HEIGHT] == 0)
        prefix = number(page, PREFIX, 2)
        assert prefix <= 1 + width + ROW_BYTES
        entries = []
        used = 0
        for end in range(1, CAPACITY + 1):
            if not page[BITMAP + end // 8] & (1 << (end % 8)):
                continue
            payload = page[ENTRY + used:ENTRY + end]
            full = payload if not entries else entries[0][:prefix] + payload
            assert len(full) == 1 + width + ROW_BYTES + (0 if leaf else CHILD_BYTES)
            entries.append(full)
            used = end
        assert entries and number(page, FREE, 2) == CAPACITY - used
        pages[number_value] = {
            'height': page[HEIGHT], 'previous': number(page, PREVIOUS),
            'next': number(page, NEXT), 'tail': number(page, TAIL),
            'entries': entries,
        }
        if leaf:
            assert number(page, TAIL) == 0
            leaves.append(number_value)
            return entries
        keys = []
        for bound in entries:
            child = int.from_bytes(bound[-CHILD_BYTES:], 'big')
            nested = visit(child, depth + 1)
            assert nested[-1] == bound[:-CHILD_BYTES]
            assert pages[child]['height'] + 1 == page[HEIGHT]
            keys.extend(nested)
        if number(page, TAIL):
            child = number(page, TAIL)
            nested = visit(child, depth + 1)
            assert pages[child]['height'] + 1 == page[HEIGHT]
            keys.extend(nested)
        return keys

    entries = visit(root, 0)
    assert entries == sorted(entries)
    for position, page in enumerate(leaves):
        assert pages[page]['previous'] == (leaves[position - 1] if position else 0)
        assert pages[page]['next'] == (leaves[position + 1] if position + 1 < len(leaves) else 0)
    decoded = [decode_key(entry, width, unsigned_byte, descending) for entry in entries]
    return pages, leaves, decoded, entries


def expected_image(case, original):
    assert len(original) == case['source']['bytes']
    assert hashlib.sha256(original).hexdigest() == case['source']['sha256']
    assert case['width'] == 8 and case['native_type'] in (5, 19)
    field = case['selected_field_offset']
    assert original[field:field + 8] == bytes(8)
    expected = bytearray(original)
    expected[field:field + 8] = case['replacement_units'].to_bytes(8, 'little', signed=True)
    allocation = len(original) // PAGE
    trees = []
    allocated_pages = []
    for index in case['physical_indexes']:
        root = index['root']
        source_entries = [bytes.fromhex(entry) for entry in index['original_entry_hex']]
        decoded_source = [decode_key(entry, 8, False, index['descending']) for entry in source_entries]
        assert all(units == 0 for units, _ in decoded_source)
        assert len(decoded_source) == case['rows'] == 900
        changed = []
        for units, reference in decoded_source:
            value = case['replacement_units'] if reference == case['selected_reference'] else units
            changed.append(encode_key(value, reference, 8, False, index['descending']))
        changed.sort()
        assert layout(changed, index['original_prefix'])[1] > CAPACITY
        first_allocation = allocation
        pages, order, allocation, events = fragments(changed, allocation)
        assert len(pages) == 3
        logical_pages = [pages[position] for position in order]
        leaf_facts = []
        for position, page in enumerate(logical_pages):
            previous = logical_pages[position - 1]['number'] if position else 0
            following = logical_pages[position + 1]['number'] if position + 1 < len(logical_pages) else 0
            raw, end = packet(fresh(case['definition'], previous, following), page['entries'], 0)
            assert not any(raw[end:])
            allocated_pages.append((page['number'], raw))
            leaf_facts.append({
                'page': page['number'], 'count': len(page['entries']),
                'prefix': number(raw, PREFIX, 2), 'free': number(raw, FREE, 2),
                'previous': previous, 'next': following,
                'maximum_full_key_row': page['entries'][-1].hex(),
            })
        before_root = original[root * PAGE:(root + 1) * PAGE]
        root_header = bytearray(before_root)
        root_header[0] = 3
        root_header[HEIGHT] = 1
        put(root_header, PREVIOUS, 0)
        put(root_header, NEXT, 0)
        put(root_header, TAIL, logical_pages[-1]['number'])
        bounds = []
        for page in logical_pages[:-1]:
            bounds.append(page['entries'][-1] + page['number'].to_bytes(CHILD_BYTES, 'big'))
        root_page, root_end = packet(root_header, bounds, 0)
        expected[root * PAGE:(root + 1) * PAGE] = root_page
        assert root_page[root_end:] == before_root[root_end:]
        trees.append({
            'physical': index['physical'], 'root': root,
            'first_allocation': first_allocation, 'allocated_pages': allocation - first_allocation,
            'logical_leaves': leaf_facts, 'root_bounds': [entry.hex() for entry in bounds],
            'root_tail': logical_pages[-1]['number'], 'events': events,
            'expected_complete_entry_hex': [entry.hex() for entry in changed],
        })
    for page, raw in sorted(allocated_pages):
        assert page * PAGE == len(expected)
        expected.extend(raw)
    for edit in case['map_edits']:
        assert original[edit['offset']] == edit['before']
        expected[edit['offset']] = edit['after']
    result = bytes(expected)
    facts = {
        'whole_image_sha256': hashlib.sha256(result).hexdigest(),
        'selected_field_offset': field, 'selected_width': 8,
        'selected_new_wire_hex': result[field:field + 8].hex(),
        'file_growth': len(result) - len(original), 'trees': trees,
        'original_counter_and_unaffected_bytes_preserved': True,
        'original_root_residual_preserved': True, 'fresh_unused_bytes_zero': True,
    }
    return result, facts
