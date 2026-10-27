"""Independent PAGE record/footer and whole-image preservation checks.

This module calls no executable. It consumes already captured metadata and raw
images. Native key bytes come only from separately recorded public references.
"""
import hashlib

from slot_preview_originals import model as SLOT, proof as INDEX

from . import model as PAGE

PAGE_BYTES = 4096
SLOT_OFFSET = 14
SLOT_BYTES = 2
COUNT_OFFSET = 12
FREE_OFFSET = 2
MASK_BITS = 8
VARIABLE_BYTES = 2
POINTER_BYTES = 4
REFERENCE_SLOT_BITS = 8
HIDDEN = 0x8000
FORWARDED = 0x4000
INDEX_FREE_OFFSET = 2
INDEX_OWNER_OFFSET = 4
MAX_PAGE_ROWS = 255
INDEX_PREFIX_OFFSET = 24
INDEX_HEIGHT_OFFSET = 26
INDEX_BITMAP_OFFSET = 27
REFERENCE_BYTES = 4


def put_word(raw, offset, value):
    raw[offset:offset + SLOT_BYTES] = value.to_bytes(SLOT_BYTES, 'little')


def rebuild_record(table, row, column, replacement, raw):
    target = PAGE.final_frame(raw, row)
    before = target['body']
    field = table['columns'][column]
    selected = field['variable']
    mask_bytes = (table['max_columns'] + MASK_BITS - 1) // MASK_BITS
    old_mask = len(before) - mask_bytes
    count = PAGE.number(before, old_mask - VARIABLE_BYTES)
    offsets = []
    for position in range(count + 1):
        offsets.append(PAGE.number(before, old_mask - VARIABLE_BYTES * (position + 2)))
    start = offsets[selected]
    stop = offsets[selected + 1]
    payload_end = offsets[-1]
    old_data_end = old_mask - VARIABLE_BYTES * (count + 2)
    footer_bytes = len(before) - old_data_end
    encoded = PAGE.payload(replacement, field)
    old_width = stop - start
    following = before[stop:payload_end]
    minimum = start + len(encoded) + len(following) + footer_bytes
    physical = max(len(before), minimum)
    after = bytearray(before)
    after.extend(bytes(physical - len(before)))
    after[start:start + len(encoded)] = encoded
    after[start + len(encoded):start + len(encoded) + len(following)] = following
    new_mask = physical - mask_bytes
    after[new_mask:] = before[old_mask:]
    bit = 1 << (field['number'] % MASK_BITS)
    mask_position = new_mask + field['number'] // MASK_BITS
    if replacement[0] == 'null':
        after[mask_position] &= ~bit
    else:
        after[mask_position] |= bit
    new_count_offset = new_mask - VARIABLE_BYTES
    put_word(after, new_count_offset, count)
    for position, old_offset in enumerate(offsets):
        offset = old_offset
        if position > selected:
            offset = old_offset - old_width + len(encoded)
        put_word(after, new_count_offset - VARIABLE_BYTES * (position + 1), offset)
    return bytes(after)


def expected_data_image(before, table, selection):
    identity = str(selection['ID'])
    row = table['rows'][identity]
    column = selection['column']
    old = SLOT.value(row['values'][table['columns'][column]['position']])
    replacement = SLOT.value(selection['replacement'])
    expected = bytearray(before)
    root = PAGE.frame(before, row['page'], row['slot'])
    target = PAGE.final_frame(before, row)
    if replacement == old:
        return expected, {'mode': 'semantic-noop', 'new_target': target['start']}
    record = rebuild_record(table, row, column, replacement, before)
    assert len(record) == selection['proof']['new_physical_record_bytes']
    if len(record) == target['length']:
        expected[target['start']:target['end']] = record
        return expected, {'mode': 'fitted', 'new_target': target['start'],
                          'record_sha256': hashlib.sha256(record).hexdigest()}
    base = root['page'] * PAGE_BYTES
    count = PAGE.number(before, base + COUNT_OFFSET)
    assert count < MAX_PAGE_ROWS
    last = PAGE.frame(before, root['page'], count - 1)
    end = last['start']
    start = end - len(record)
    assert start >= base + SLOT_OFFSET + (count + 1) * SLOT_BYTES
    free = PAGE.number(before, base + FREE_OFFSET)
    assert free == end - (base + SLOT_OFFSET + count * SLOT_BYTES)
    assert free >= len(record) + SLOT_BYTES
    expected[start:end] = record
    put_word(expected, base + SLOT_OFFSET + count * SLOT_BYTES, (start - base) | HIDDEN)
    put_word(expected, base + COUNT_OFFSET, count + 1)
    put_word(expected, base + FREE_OFFSET, free - len(record) - SLOT_BYTES)
    if not root['word'] & FORWARDED:
        expected[root['start']:root['end']] = bytes(root['length'])
    reference = (root['page'] << REFERENCE_SLOT_BITS) | count
    expected[root['start']:root['start'] + POINTER_BYTES] = reference.to_bytes(POINTER_BYTES, 'little')
    put_word(expected, base + SLOT_OFFSET + root['slot'] * SLOT_BYTES, root['word'] | FORWARDED)
    return expected, {'mode': 'append', 'new_target': start, 'new_slot': count,
                      'record_sha256': hashlib.sha256(record).hexdigest()}


def verify(before_raw, after_raw, tables, actual, selection, reference=None, reference_raw=None):
    assert selection['expected_candidate']
    assert len(before_raw) == len(after_raw)
    INDEX.unchanged_metadata(tables, actual)
    name = selection['table']
    table = tables[name]
    identity = str(selection['ID'])
    column = selection['column']
    position = table['columns'][column]['position']
    noop = selection['expected'] == selection['replacement']
    for label, original in tables.items():
        assert list(original['rows']) == list(actual[label]['rows'])
        for row_id, row in original['rows'].items():
            current = actual[label]['rows'][row_id]
            values = list(row['values'])
            if label == name and str(row_id) == identity:
                values[position] = selection['replacement']
            assert current['values'] == values
            assert (current['page'], current['slot']) == (row['page'], row['slot'])
    expected, data_proof = expected_data_image(before_raw, table, selection)
    before_pages = INDEX.pages(before_raw, table)
    after_pages = INDEX.pages(after_raw, table)
    affected = set()
    if not noop:
        for index in table['indexes'].values():
            if any(component[0] == column for component in index['columns']):
                affected.add(index['root'])
    reference_pages = INDEX.pages(reference_raw, reference[name]) if affected else {}
    allowed = []
    orders = {}
    for index in table['indexes'].values():
        root = index['root']
        if root in orders:
            continue
        original_entries, visited = INDEX.tree_entries(root, before_pages)
        current_entries, current_visited = INDEX.tree_entries(root, after_pages, before_pages)
        assert current_visited == visited
        original_keys, original_order = INDEX.by_identity(table, original_entries)
        current_keys, current_order = INDEX.by_identity(table, current_entries)
        expected_keys = dict(original_keys)
        expected_order = original_order
        if root in affected:
            assert reference is not None and reference_raw is not None
            source = next(item for item in reference[name]['indexes'].values()
                          if item['physical'] == index['physical'])
            public_entries, _ = INDEX.tree_entries(source['root'], reference_pages)
            public_keys, expected_order = INDEX.by_identity(reference[name], public_entries)
            selected = selection['ID']
            expected_keys[selected] = public_keys[selected][:-REFERENCE_BYTES] + original_keys[selected][-REFERENCE_BYTES:]
            for number in visited:
                start = number * PAGE_BYTES
                assert after_raw[start:start + INDEX_FREE_OFFSET] == before_raw[start:start + INDEX_FREE_OFFSET]
                assert after_raw[start + INDEX_OWNER_OFFSET:start + INDEX_PREFIX_OFFSET] == before_raw[start + INDEX_OWNER_OFFSET:start + INDEX_PREFIX_OFFSET]
                assert after_raw[start + INDEX_HEIGHT_OFFSET] == before_raw[start + INDEX_HEIGHT_OFFSET]
                allowed.extend([
                    (start + INDEX_FREE_OFFSET, start + INDEX_FREE_OFFSET + SLOT_BYTES),
                    (start + INDEX_PREFIX_OFFSET, start + INDEX_PREFIX_OFFSET + SLOT_BYTES),
                    (start + INDEX_BITMAP_OFFSET, start + PAGE_BYTES),
                ])
        assert current_keys == expected_keys
        assert current_order == expected_order
        orders[root] = expected_order
    for label, original in tables.items():
        for index_name, index in original['indexes'].items():
            expected_order = orders[index['root']] if label == name else index['order']
            assert actual[label]['indexes'][index_name]['order'] == expected_order
    allowed.sort()
    cursor = 0
    for offset, (wanted, actual_byte) in enumerate(zip(expected, after_raw)):
        if wanted == actual_byte:
            continue
        while cursor < len(allowed) and allowed[cursor][1] <= offset:
            cursor += 1
        assert cursor < len(allowed) and allowed[cursor][0] <= offset, offset
    if noop:
        assert after_raw == before_raw
    data_proof.update({'whole_image_outside_exact_index_spans_matches': True,
                       'original_root_and_all_logical_row_locations_preserved': True,
                       'all_typed_values_and_logical_index_orders_match': True,
                       'all_unselected_and_retired_target_bytes_preserved': True,
                       'all_original_index_topology_and_full_maxima_checked': True})
    return data_proof
