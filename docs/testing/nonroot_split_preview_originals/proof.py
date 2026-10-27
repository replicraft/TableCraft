"""Literal original-image expectation for the frozen two non-root requests."""
import hashlib

PAGE_BYTES = 4096
ENTRY_OFFSET = 480
BITMAP_OFFSET = 27
FREE_OFFSET = 2
OWNER_OFFSET = 4
PREVIOUS_OFFSET = 12
NEXT_OFFSET = 16
TAIL_OFFSET = 20
PREFIX_OFFSET = 24
HEIGHT_OFFSET = 26
ROW_BYTES = 4
CHILD_BYTES = 4


def put(raw, at, value, width, order='little'):
    raw[at:at + width] = value.to_bytes(width, order)


def packet(initial, entries, prefix):
    result = bytearray(initial)
    result[BITMAP_OFFSET:ENTRY_OFFSET] = bytes(ENTRY_OFFSET - BITMAP_OFFSET)
    length = 0
    for position, entry in enumerate(entries):
        payload = entry if position == 0 else entry[prefix:]
        result[ENTRY_OFFSET + length:ENTRY_OFFSET + length + len(payload)] = payload
        length += len(payload)
        result[BITMAP_OFFSET + length // 8] |= 1 << (length % 8)
    assert length <= PAGE_BYTES - ENTRY_OFFSET
    put(result, FREE_OFFSET, PAGE_BYTES - ENTRY_OFFSET - length, 2)
    put(result, PREFIX_OFFSET, prefix, 2)
    return bytes(result), ENTRY_OFFSET + length


def expected_image(case, original):
    assert len(original) == case['original']['bytes']
    assert hashlib.sha256(original).hexdigest() == case['original']['sha256']
    entries = [bytes.fromhex(value) for value in case['affected_original_entry_hex']]
    assert len(entries) == 496 and entries == sorted(entries)
    last = entries.pop()
    assert last.hex() == '7f800003e8' + case['original_reference_hex']
    entries.append(bytes.fromhex('7fffffffff') + last[-ROW_BYTES:])
    assert entries == sorted(entries)
    left_keys = entries[:248]
    right_keys = entries[248:]
    root = case['root']
    old_right = case['retained_right']
    previous = case['previous_leaf']
    new_left = len(original) // PAGE_BYTES
    assert new_left == case['new_left_eof']
    expected = bytearray(original)
    field = case['selected_field_offset']
    assert original[field:field + 4] == (1000).to_bytes(4, 'little')
    expected[field:field + 4] = (2147483647).to_bytes(4, 'little')
    root_entries = [bytes.fromhex(value) for value in case['original_root_entry_hex']]
    assert len(root_entries) == 1
    root_entries.append(left_keys[-1] + new_left.to_bytes(CHILD_BYTES, 'big'))
    root_raw = original[root * PAGE_BYTES:(root + 1) * PAGE_BYTES]
    rebuilt_root, root_end = packet(root_raw, root_entries, 0)
    assert int.from_bytes(rebuilt_root[TAIL_OFFSET:TAIL_OFFSET + 4], 'little') == old_right
    assert rebuilt_root[HEIGHT_OFFSET] == 1
    expected[root * PAGE_BYTES:(root + 1) * PAGE_BYTES] = rebuilt_root
    right_raw = bytearray(original[old_right * PAGE_BYTES:(old_right + 1) * PAGE_BYTES])
    put(right_raw, PREVIOUS_OFFSET, new_left, 4)
    rebuilt_right, right_end = packet(right_raw, right_keys, 1)
    expected[old_right * PAGE_BYTES:(old_right + 1) * PAGE_BYTES] = rebuilt_right
    put(expected, previous * PAGE_BYTES + NEXT_OFFSET, new_left, 4)
    fresh = bytearray(PAGE_BYTES)
    fresh[:2] = bytes([4, 1])
    put(fresh, OWNER_OFFSET, case['definition'], 4)
    put(fresh, PREVIOUS_OFFSET, previous, 4)
    put(fresh, NEXT_OFFSET, old_right, 4)
    rebuilt_left, left_end = packet(fresh, left_keys, 0)
    expected.extend(rebuilt_left)
    for edit in case['map_edits']:
        assert expected[edit['offset']] == edit['before']
        expected[edit['offset']] = edit['after']
    assert rebuilt_root[root_end:] == root_raw[root_end:]
    assert rebuilt_right[right_end:] == original[old_right * PAGE_BYTES + right_end:(old_right + 1) * PAGE_BYTES]
    assert not any(rebuilt_left[left_end:])
    facts = {
        'retained_root': root,
        'root_height': 1,
        'retained_right': old_right,
        'new_left_eof': new_left,
        'previous_leaf': previous,
        'left_entries': len(left_keys),
        'right_entries': len(right_keys),
        'left_maximum_full_key_row': left_keys[-1].hex(),
        'right_maximum_full_key_row': right_keys[-1].hex(),
        'existing_root_entries': len(root_entries),
        'root_residual_start': root * PAGE_BYTES + root_end,
        'right_residual_start': old_right * PAGE_BYTES + right_end,
        'whole_expected_image_sha256': hashlib.sha256(expected).hexdigest(),
        'file_growth': PAGE_BYTES,
        'original_LVAL_descriptors_and_pages_preserved': True,
    }
    return bytes(expected), facts


def prove(case, original, actual):
    expected, facts = expected_image(case, original)
    assert expected == actual, 'Exact non-root whole-image authority'
    return facts
