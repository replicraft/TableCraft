"""Independent complete-image index-map promotion expectation, before candidate calls."""
import hashlib

PAGE = 4096
ENTRY_START = 480
ROW_BYTES = 4
MAP_BYTES = 69
BITMAP_HEADER = 4
INLINE_BITS = 512


def number(raw, offset, width=4):
    return int.from_bytes(raw[offset:offset + width], 'little')


def put(raw, offset, value, width=4):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def key(value, reference):
    assert -(1 << 31) <= value < (1 << 31)
    ordered = ((value % (1 << 32)) ^ (1 << 31)).to_bytes(4, 'big')
    return b'\x7f' + ordered + reference.to_bytes(ROW_BYTES, 'big')


def common_prefix(entries):
    assert entries
    prefix = len(entries[0])
    for entry in entries[1:]:
        prefix = min(prefix, len(entry))
        for position in range(prefix):
            if entries[0][position] != entry[position]:
                prefix = position
                break
    return prefix


def packet(initial, entries):
    result = bytearray(initial)
    result[27:ENTRY_START] = bytes(ENTRY_START - 27)
    total = sum(len(entry) for entry in entries)
    prefix = 0
    if total > PAGE - ENTRY_START:
        prefix = common_prefix(entries)
    used = 0
    for position, entry in enumerate(entries):
        suffix = entry if position == 0 else entry[prefix:]
        end = used + len(suffix)
        assert end <= PAGE - ENTRY_START
        result[ENTRY_START + used:ENTRY_START + end] = suffix
        result[27 + end // 8] |= 1 << (end % 8)
        used = end
    put(result, 2, PAGE - ENTRY_START - used, 2)
    put(result, 24, prefix, 2)
    return bytes(result), ENTRY_START + used


def leaf(owner, previous, following, entries):
    raw = bytearray(PAGE)
    raw[:2] = b'\x04\x01'
    put(raw, 4, owner)
    put(raw, 12, previous)
    put(raw, 16, following)
    result, end = packet(raw, entries)
    assert not any(result[end:])
    return result


def expected_image(case, original):
    assert len(original) == case['source']['bytes']
    assert hashlib.sha256(original).hexdigest() == case['source']['sha256']
    frontier = len(original) // PAGE
    assert frontier == case['eof']
    right, bitmap_page, left = frontier, frontier + 1, frontier + 2
    owner, root = case['definition'], case['root']
    assert max(root, right, left) - min(root, right, left) + 1 > INLINE_BITS

    source_entries = [bytes.fromhex(entry) for entry in case['source_entry_hex']]
    assert len(source_entries) == 900
    assert all(entry[:5] == key(0, 0)[:5] for entry in source_entries)
    references = [int.from_bytes(entry[-ROW_BYTES:], 'big') for entry in source_entries]
    assert len(set(references)) == 900
    assert case['selected_reference'] in references
    entries = sorted(key(2147483647 if ref == case['selected_reference'] else 0, ref)
                     for ref in references)
    assert common_prefix(entries) == 1
    assert len(entries) * 9 - (len(entries) - 1) > PAGE - ENTRY_START
    first, second = entries[:450], entries[450:]

    expected = bytearray(original)
    field = case['selected_field_offset']
    assert original[field:field + 4] == bytes(4)
    expected[field:field + 4] = (2147483647).to_bytes(4, 'little')

    old_root = original[root * PAGE:(root + 1) * PAGE]
    assert old_root[:2] == b'\x04\x01'
    assert number(old_root, 4) == owner and old_root[26] == 0
    root_header = bytearray(old_root)
    root_header[0] = 3
    root_header[26] = 1
    put(root_header, 12, 0)
    put(root_header, 16, 0)
    put(root_header, 20, right)
    root_page, root_end = packet(root_header, [first[-1] + left.to_bytes(4, 'big')])
    assert root_page[root_end:] == old_root[root_end:]
    expected[root * PAGE:(root + 1) * PAGE] = root_page

    right_page = leaf(owner, left, 0, second)
    left_page = leaf(owner, 0, right, first)
    bitmap = bytearray(PAGE)
    bitmap[:BITMAP_HEADER] = b'\x05\x01\x00\x00'
    for member in (root, right, left):
        bitmap[BITMAP_HEADER + member // 8] |= 1 << (member % 8)
    assert not bitmap[BITMAP_HEADER + bitmap_page // 8] & (1 << (bitmap_page % 8))

    start, end = case['owned_map_range']
    assert end - start == MAP_BYTES and original[start] == 0
    assert list(case['original_members']) == [root]
    owned = bytearray(MAP_BYTES)
    owned[0] = 1
    put(owned, 1, bitmap_page)
    expected[start:end] = owned
    for change in case['global_edits']:
        assert original[change['offset']] == change['before']
        expected[change['offset']] = change['after']
    expected.extend(right_page)
    expected.extend(bitmap)
    expected.extend(left_page)
    assert len(expected) == len(original) + 3 * PAGE

    return bytes(expected), {
        'whole_image_sha256': hashlib.sha256(expected).hexdigest(),
        'file_growth': 3 * PAGE,
        'index_pages': [right, left],
        'metadata_pages': [bitmap_page],
        'root': root,
        'root_bound': (first[-1] + left.to_bytes(4, 'big')).hex(),
        'root_tail': right,
        'left_prefix': number(left_page, 24, 2),
        'right_prefix': number(right_page, 24, 2),
        'left_count': len(first),
        'right_count': len(second),
        'fresh_unused_bytes_zero': True,
        'root_residue_preserved': True,
        'owned_map_hex': owned.hex(),
        'bitmap_sha256': hashlib.sha256(bitmap).hexdigest(),
        'complete_entries_hex': [entry.hex() for entry in entries],
    }
