"""Compare captured final rows and raw topology; no assumption of root growth."""
import hashlib

PAGE_BYTES = 4096
ROW_DIRECTORY = 14
SLOT_BYTES = 2
OFFSET_MASK = 0x1FFF
SELECTED = 149184
REPLACEMENT = 2147483647
KEY_START = 2
KEY_BYTES = 4
REFERENCE_BYTES = 4


def body(raw, row):
    start = row['page'] * PAGE_BYTES
    slot = row['slot']
    word = int.from_bytes(raw[start + ROW_DIRECTORY + slot * SLOT_BYTES:
                              start + ROW_DIRECTORY + (slot + 1) * SLOT_BYTES], 'little')
    low = start + (word & OFFSET_MASK)
    if slot:
        prior = int.from_bytes(raw[start + ROW_DIRECTORY + (slot - 1) * SLOT_BYTES:
                                   start + ROW_DIRECTORY + slot * SLOT_BYTES], 'little')
        high = start + (prior & OFFSET_MASK)
    else:
        high = start + PAGE_BYTES
    return {'word': word, 'offset': low, 'bytes_hex': raw[low:high].hex()}


def compare(before_raw, after_raw, before_tables, after_tables, before_maps, after_maps, wire):
    expected_rows = dict(before_tables['SparseInt32']['rows'])
    selected = expected_rows.pop(SELECTED)
    expected_rows[REPLACEMENT] = selected
    actual_rows = after_tables['SparseInt32']['rows']
    assert set(expected_rows) == set(actual_rows)
    for value, expected in expected_rows.items():
        assert actual_rows[value]['marker'] == expected['marker']
    assert before_tables['Sentinel'] == after_tables['Sentinel']
    prior_table = before_tables['SparseInt32']
    later_table = after_tables['SparseInt32']
    assert prior_table['columns'] == later_table['columns']
    assert prior_table['count'] == later_table['count'] == 963
    prior_index = prior_table['indexes']['PrimaryKey']
    later_index = later_table['indexes']['PrimaryKey']
    for field in ('physical', 'primary', 'required', 'unique', 'ignore_nulls', 'columns'):
        assert prior_index[field] == later_index[field]
    before_nodes, before_leaves, before_entries = wire.tree(
        before_raw, prior_index['root'], prior_table['owner'],
    )
    after_nodes, after_leaves, after_entries = wire.tree(
        after_raw, later_index['root'], later_table['owner'],
    )
    expected_keys = list(before_entries)
    reference = expected_keys[-1][-REFERENCE_BYTES:]
    assert wire.key(expected_keys[-1]) == SELECTED
    expected_keys[-1] = wire.encoded(REPLACEMENT, reference)
    assert after_entries == expected_keys
    assert [wire.key(entry) for entry in after_entries] == later_index['order']
    for entry in after_entries:
        row = actual_rows[wire.key(entry)]
        assert int.from_bytes(entry[-REFERENCE_BYTES:], 'big') == (row['page'] << 8) | row['slot']
    records = []
    for value, row in prior_table['rows'].items():
        new_value = REPLACEMENT if value == SELECTED else value
        new_row = actual_rows[new_value]
        before = body(before_raw, row)
        after = body(after_raw, new_row)
        expected = bytes.fromhex(before['bytes_hex'])
        if value == SELECTED:
            expected = expected[:KEY_START] + REPLACEMENT.to_bytes(KEY_BYTES, 'little') + expected[KEY_START + KEY_BYTES:]
        records.append({'original_value': value, 'before_reference': [row['page'], row['slot']],
                        'after_reference': [new_row['page'], new_row['slot']],
                        'same_root_reference': (row['page'], row['slot']) == (new_row['page'], new_row['slot']),
                        'expected_fixed_patch_or_unchanged_body': bytes.fromhex(after['bytes_hex']) == expected,
                        'before': before, 'after': after})
    transitions = {}
    for name, original in before_maps.items():
        final = after_maps[name]
        cells = dict(original['bit_locations'])
        cells.update(final['bit_locations'])
        offsets = sorted({cell['offset'] for cell in cells.values() if cell['offset'] < len(before_raw)})
        transitions[name] = {
            'added_file_members': sorted(set(final['file_members']) - set(original['file_members'])),
            'removed_file_members': sorted(set(original['file_members']) - set(final['file_members'])),
            'existing_byte_changes': [{'offset': offset, 'before': before_raw[offset], 'after': after_raw[offset]}
                                      for offset in offsets if before_raw[offset] != after_raw[offset]],
        }
    def describe(nodes, leaves):
        return {'nodes': {str(number): {
            **{key: value for key, value in node.items() if key != 'entries'},
            'entries_hex': [entry.hex() for entry in node['entries']],
        } for number, node in nodes.items()}, 'leaves': leaves}
    return {
        'all_typed_rows_and_markers_verified': True,
        'all_complete_key_row_references_verified': True,
        'root_before': prior_index['root'], 'root_after': later_index['root'],
        'root_height_before': before_nodes[prior_index['root']]['height'],
        'root_height_after': after_nodes[later_index['root']]['height'],
        'actual_root_growth': after_nodes[later_index['root']]['height'] > before_nodes[prior_index['root']]['height'],
        'root_id_retained': prior_index['root'] == later_index['root'],
        'before_tree': describe(before_nodes, before_leaves),
        'after_tree': describe(after_nodes, after_leaves),
        'added_index_pages': sorted(set(after_nodes) - set(before_nodes)),
        'removed_index_pages': sorted(set(before_nodes) - set(after_nodes)),
        'existing_changed_pages': [page for page in range(len(before_raw) // PAGE_BYTES)
                                   if before_raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                                   != after_raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]],
        'eof_added_pages': list(range(len(before_raw) // PAGE_BYTES, len(after_raw) // PAGE_BYTES)),
        'map_transitions': transitions, 'records': records,
        'historical_counter_before_after': [prior_index['historical'], later_index['historical']],
        'file_bytes_before_after': [len(before_raw), len(after_raw)],
        'public_unrelated_byte_changes_observed_not_kernel_goldens': True,
    }
