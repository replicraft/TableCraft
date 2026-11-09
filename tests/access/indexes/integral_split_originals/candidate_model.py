"""Literal exact-width numeric keys and whole-image root-leaf split expectations."""
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


def packet(initial, entries):
    # All new halves in this corpus fit without prefix compression. The retained
    # root's single complete separator also has prefix zero by contract.
    result = bytearray(initial)
    result[BITMAP:ENTRY] = bytes(ENTRY - BITMAP)
    used = 0
    for entry in entries:
        assert used + len(entry) <= CAPACITY
        result[ENTRY + used:ENTRY + used + len(entry)] = entry
        used += len(entry)
        result[BITMAP + used // 8] |= 1 << (used % 8)
    put(result, FREE, CAPACITY - used, 2)
    put(result, PREFIX, 0, 2)
    return bytes(result), ENTRY + used


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
    width = case['width']
    unsigned_byte = case['native_type'] == 2
    field = case['selected_field_offset']
    assert original[field:field + width] == bytes(width)
    expected = bytearray(original)
    expected[field:field + width] = case['replacement_units'].to_bytes(width, 'little', signed=not unsigned_byte)
    allocation = len(original) // PAGE
    trees = []
    for index in case['physical_indexes']:
        root = index['root']
        source_entries = [bytes.fromhex(entry) for entry in index['original_entry_hex']]
        decoded_source = [decode_key(entry, width, unsigned_byte, index['descending']) for entry in source_entries]
        assert all(units == 0 for units, _ in decoded_source)
        assert len(decoded_source) == case['rows']
        changed = [encode_key(case['replacement_units'] if reference == case['selected_reference'] else units,
                              reference, width, unsigned_byte, index['descending'])
                   for units, reference in decoded_source]
        changed.sort()
        middle = len(changed) // 2
        left, right = changed[:middle], changed[middle:]
        right_number, left_number = allocation, allocation + 1
        allocation += 2
        right_page, right_end = packet(fresh(case['definition'], left_number, 0), right)
        left_page, left_end = packet(fresh(case['definition'], 0, right_number), left)
        expected.extend(right_page)
        expected.extend(left_page)
        before_root = original[root * PAGE:(root + 1) * PAGE]
        root_header = bytearray(before_root)
        root_header[0] = 3
        root_header[HEIGHT] = 1
        put(root_header, PREVIOUS, 0)
        put(root_header, NEXT, 0)
        put(root_header, TAIL, right_number)
        separator = left[-1] + left_number.to_bytes(CHILD_BYTES, 'big')
        root_page, root_end = packet(root_header, [separator])
        expected[root * PAGE:(root + 1) * PAGE] = root_page
        assert root_page[root_end:] == before_root[root_end:]
        assert not any(right_page[right_end:]) and not any(left_page[left_end:])
        trees.append({
            'physical': index['physical'], 'root': root,
            'allocated_right': right_number, 'allocated_left': left_number,
            'left_entries': len(left), 'right_entries': len(right),
            'left_maximum_full_key_row': left[-1].hex(),
            'right_maximum_full_key_row': right[-1].hex(),
            'expected_complete_entry_hex': [entry.hex() for entry in changed],
        })
    for edit in case['map_edits']:
        assert original[edit['offset']] == edit['before']
        expected[edit['offset']] = edit['after']
    result = bytes(expected)
    facts = {
        'whole_image_sha256': hashlib.sha256(result).hexdigest(),
        'selected_field_offset': field, 'selected_width': width,
        'selected_new_wire_hex': result[field:field + width].hex(),
        'file_growth': len(result) - len(original), 'trees': trees,
        'original_counter_and_unaffected_bytes_preserved': True,
        'original_root_residual_preserved': True, 'fresh_unused_bytes_zero': True,
    }
    return result, facts
