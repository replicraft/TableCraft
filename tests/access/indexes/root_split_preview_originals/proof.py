"""Independent expected images for two frozen original Int32 split selectors."""
import hashlib

PAGE_BYTES = 4096
FREE_OFFSET = 2
OWNER_OFFSET = 4
PREVIOUS_OFFSET = 12
NEXT_OFFSET = 16
TAIL_OFFSET = 20
PREFIX_OFFSET = 24
HEIGHT_OFFSET = 26
BITMAP_OFFSET = 27
ENTRY_OFFSET = 480
ROW_BYTES = 4
CHILD_BYTES = 4
NODE_KIND = 3
LEAF_KIND = 4
EXPECTED_ROWS = 500
REPLACEMENT = 2147483647


def put(raw, offset, value, width, order='little'):
    raw[offset:offset + width] = value.to_bytes(width, order)


def packet(initial, entries):
    result = bytearray(initial)
    result[BITMAP_OFFSET:ENTRY_OFFSET] = bytes(ENTRY_OFFSET - BITMAP_OFFSET)
    cursor = 0
    for entry in entries:
        result[ENTRY_OFFSET + cursor:ENTRY_OFFSET + cursor + len(entry)] = entry
        cursor += len(entry)
        result[BITMAP_OFFSET + cursor // 8] |= 1 << (cursor % 8)
    assert cursor <= PAGE_BYTES - ENTRY_OFFSET
    put(result, FREE_OFFSET, PAGE_BYTES - ENTRY_OFFSET - cursor, 2)
    put(result, PREFIX_OFFSET, 0, 2)
    return bytes(result)


def leaf(owner, previous, following, entries):
    raw = bytearray(PAGE_BYTES)
    raw[0] = LEAF_KIND
    raw[1] = 1
    put(raw, OWNER_OFFSET, owner, 4)
    put(raw, PREVIOUS_OFFSET, previous, 4)
    put(raw, NEXT_OFFSET, following, 4)
    return packet(raw, entries)


def expected_image(case, original):
    assert len(original) == case['original']['bytes']
    assert hashlib.sha256(original).hexdigest() == case['original']['sha256']
    prior = [bytes.fromhex(value) for value in case['original_entry_hex']]
    assert len(prior) == EXPECTED_ROWS
    assert len(set(value[-ROW_BYTES:] for value in prior)) == EXPECTED_ROWS
    assert all(len(value) == 9 for value in prior)
    assert [int.from_bytes(value[1:5], 'big') - (1 << 31) for value in prior] == list(range(1, EXPECTED_ROWS + 1))
    removed = prior.pop(0)
    assert removed[-ROW_BYTES:].hex() == case['original_record_row_reference_hex']
    replacement = bytes.fromhex('7fffffffff') + removed[-ROW_BYTES:]
    final = sorted([*prior, replacement])
    assert sorted(value[-ROW_BYTES:] for value in final) == sorted(value[-ROW_BYTES:] for value in [removed, *prior])
    middle = len(final) // 2
    left_entries = final[:middle]
    right_entries = final[middle:]
    assert len(left_entries) == len(right_entries) == 250
    right = len(original) // PAGE_BYTES
    left = right + 1
    root = case['primary_root']
    low = root * PAGE_BYTES
    original_root = original[low:low + PAGE_BYTES]
    assert original_root[0] == LEAF_KIND and original_root[HEIGHT_OFFSET] == 0
    root_page = bytearray(original_root)
    root_page[0] = NODE_KIND
    root_page[HEIGHT_OFFSET] = 1
    put(root_page, TAIL_OFFSET, right, 4)
    separator = left_entries[-1] + left.to_bytes(CHILD_BYTES, 'big')
    root_page = packet(root_page, [separator])
    image = bytearray(original)
    image.extend(leaf(case['definition'], left, 0, right_entries))
    image.extend(leaf(case['definition'], 0, right, left_entries))
    image[low:low + PAGE_BYTES] = root_page
    offset = case['selected_field_offset']
    assert original[offset:offset + 4] == (1).to_bytes(4, 'little')
    image[offset:offset + 4] = REPLACEMENT.to_bytes(4, 'little')
    for cell in case['map_edits']:
        assert image[cell['offset']] == cell['before']
        image[cell['offset']] = cell['after']
    facts = {
        'root': root,
        'height': 1,
        'right_eof_first': right,
        'left_eof_second': left,
        'left_count': len(left_entries),
        'right_count': len(right_entries),
        'left_maximum_full_key_row_hex': left_entries[-1].hex(),
        'right_maximum_full_key_row_hex': right_entries[-1].hex(),
        'all_500_reference_identities_preserved': True,
        'full_key_row_sha256': hashlib.sha256(b''.join(final)).hexdigest(),
        'root_residual_start': low + ENTRY_OFFSET + len(separator),
        'whole_expected_image_sha256': hashlib.sha256(image).hexdigest(),
    }
    assert image[facts['root_residual_start']:low + PAGE_BYTES] == original[facts['root_residual_start']:low + PAGE_BYTES]
    for number in [left, right]:
        padding_start = number * PAGE_BYTES + ENTRY_OFFSET + middle * len(final[0])
        assert not any(image[padding_start:(number + 1) * PAGE_BYTES])
    return bytes(image), facts


def prove(case, original, actual):
    expected, facts = expected_image(case, original)
    assert actual == expected, 'Complete expected-image proof: unauthorized byte or incorrect split'
    return facts
