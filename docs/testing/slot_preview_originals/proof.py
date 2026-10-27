"""Independent byte/row/index checks; uses only read-only reference decoders."""
import capture_delete_trees as TREE

PAGE_BYTES = 4096
NODE_KIND = 3
LEAF_KIND = 4
FREE_SPACE_OFFSET = 2
FREE_SPACE_BYTES = 2
OWNER_OFFSET = 4
COMMON_PREFIX_OFFSET = 24
COMMON_PREFIX_BYTES = 2
HEIGHT_OFFSET = 26
ENTRY_MAP_OFFSET = 27
ENTRY_MAP_BYTES = 453
REFERENCE_BYTES = 4
CHILD_BYTES = 4
REFERENCE_SLOT_BITS = 8


def pages(raw, table):
    return TREE.pages(raw, {'geometry': {
        'page_bytes': PAGE_BYTES, 'definition_page': table['definition'],
        'mask_offset': ENTRY_MAP_OFFSET, 'mask_bytes': ENTRY_MAP_BYTES,
    }})


def tree_entries(root, decoded, original=None):
    visited = set()

    def walk(number):
        assert number not in visited
        visited.add(number)
        page = decoded[number]
        entries = [bytes.fromhex(value) for value in page['entries_hex']]
        if original is not None:
            old = original[number]
            assert page['kind'] == old['kind']
            assert page['height'] == old['height']
            assert page['tail'] == old['tail']
            if page['kind'] == LEAF_KIND:
                assert len(entries) == len(old['entries_hex'])
            else:
                children = [entry[-CHILD_BYTES:] for entry in entries]
                old_children = [bytes.fromhex(value)[-CHILD_BYTES:] for value in old['entries_hex']]
                assert children == old_children
        if page['kind'] == LEAF_KIND:
            assert entries
            return entries
        assert page['kind'] == NODE_KIND
        rows = []
        for separator in entries:
            child = int.from_bytes(separator[-CHILD_BYTES:], 'big')
            child_rows = walk(child)
            assert child_rows and separator[:-CHILD_BYTES] == child_rows[-1]
            rows.extend(child_rows)
        if page['tail']:
            rows.extend(walk(page['tail']))
        assert rows
        return rows

    return walk(root), visited


def by_identity(table, entries):
    identities = {(row['page'] << REFERENCE_SLOT_BITS) | row['slot']: int(identity)
                  for identity, row in table['rows'].items()}
    result = {}
    order = []
    for entry in entries:
        identity = identities[int.from_bytes(entry[-REFERENCE_BYTES:], 'big')]
        assert identity not in result
        result[identity] = entry
        order.append(identity)
    return result, order


def unchanged_metadata(before, actual):
    assert set(before) == set(actual)
    for name, table in before.items():
        current = actual[name]
        for field in ['count', 'definition', 'columns_count', 'max_columns', 'properties']:
            assert table[field] == current[field], (name, field)
        assert set(table['columns']) == set(current['columns'])
        for column, definition in table['columns'].items():
            for field, value in current['columns'][column].items():
                assert definition[field] == value, (name, column, field)
        assert set(table['indexes']) == set(current['indexes'])
        for label, index in table['indexes'].items():
            for field in [
                'physical',
                'root',
                'entries',
                'owned_pages',
                'primary',
                'unique',
                'required',
                'ignored_null',
                'columns',
            ]:
                assert index[field] == current['indexes'][label][field], (name, label, field)


def verify(before_raw, after_raw, original, actual, selection, reference, reference_raw):
    assert len(before_raw) == len(after_raw)
    tables = original['tables']
    unchanged_metadata(tables, actual)
    name = selection['table']
    identity = str(selection['ID'])
    column = selection['column']
    table = tables[name]
    field = table['columns'][column]
    old_row = table['rows'][identity]
    selected_position = field['position']
    replacement = selection['proof']['replacement_getter']
    noop = selection['proof']['old_getter'] == replacement
    for label, old_table in tables.items():
        assert list(old_table['rows']) == list(actual[label]['rows'])
        for row_id, row in old_table['rows'].items():
            expected = list(row['values'])
            if label == name and str(row_id) == identity:
                expected[selected_position] = replacement
            assert actual[label]['rows'][row_id]['values'] == expected, (label, row_id)
            current_row = actual[label]['rows'][row_id]
            assert (current_row['page'], current_row['slot']) == (row['page'], row['slot'])
    slot = old_row['slots'][column]
    row_body = bytes.fromhex(table['raw_rows'][identity])
    row_start = slot['absolute_start'] - slot['row_start']
    mask_offset = row_start + len(row_body) - (table['max_columns'] + 7) // 8
    mask_byte = mask_offset + field['number'] // 8
    bit = 1 << (field['number'] % 8)
    present = selection['replacement_kind'] != 'null'
    expected_mask = before_raw[mask_byte] | bit if present else before_raw[mask_byte] & ~bit
    assert after_raw[mask_byte] == expected_mask
    wire = after_raw[slot['absolute_start']:slot['absolute_end']].hex()
    assert wire in selection['proof']['legal_representations']
    affected = [index for index in table['indexes'].values()
                if any(component[0] == column for component in index['columns'])]
    affected_roots = {index['root'] for index in affected} if not noop else set()
    before_pages = pages(before_raw, table)
    after_pages = pages(after_raw, table)
    reference_pages = pages(reference_raw, reference[name]) if affected_roots else {}
    allowances = [(slot['absolute_start'], slot['absolute_end']), (mask_byte, mask_byte + 1)]
    expected_orders = {}
    page_count = 0
    physical_count = 0
    for index in table['indexes'].values():
        root = index['root']
        if root in expected_orders:
            continue
        old_entries, visited = tree_entries(root, before_pages)
        actual_entries, current_visited = tree_entries(root, after_pages, before_pages)
        assert visited == current_visited
        old_keys, old_order = by_identity(table, old_entries)
        actual_keys, actual_order = by_identity(table, actual_entries)
        assert set(actual_keys) == set(old_keys)
        expected_keys = dict(old_keys)
        expected_order = old_order
        if root in affected_roots:
            assert selection['reference_reopened_requested_value'] is True
            # Public key bytes are a semantic oracle only. Reuse the original
            # row reference, never the public writer's row relocation/padding.
            reference_index = next(item for item in reference[name]['indexes'].values()
                                   if item['physical'] == index['physical'])
            public_entries, _ = tree_entries(reference_index['root'], reference_pages)
            public_keys, expected_order = by_identity(reference[name], public_entries)
            selected = selection['ID']
            expected_keys[selected] = public_keys[selected][:-REFERENCE_BYTES] + old_keys[selected][-REFERENCE_BYTES:]
            for number in visited:
                start = number * PAGE_BYTES
                assert after_raw[start: start + FREE_SPACE_OFFSET] == before_raw[start:start + FREE_SPACE_OFFSET]
                header = slice(start + OWNER_OFFSET, start + COMMON_PREFIX_OFFSET)
                assert after_raw[header] == before_raw[header]
                assert after_raw[start + HEIGHT_OFFSET] == before_raw[start + HEIGHT_OFFSET]
                allowances.extend([
                    (start + FREE_SPACE_OFFSET, start + FREE_SPACE_OFFSET + FREE_SPACE_BYTES),
                    (start + COMMON_PREFIX_OFFSET, start + COMMON_PREFIX_OFFSET + COMMON_PREFIX_BYTES),
                    (start + ENTRY_MAP_OFFSET, start + PAGE_BYTES),
                ])
            page_count += len(visited)
            physical_count += 1
        assert actual_keys == expected_keys
        assert actual_order == expected_order
        expected_orders[root] = actual_order
    # Compare every logical alias and every unrelated table index as well.
    for label, old_table in tables.items():
        for index_name, index in old_table['indexes'].items():
            expected = expected_orders[index['root']] if label == name else index['order']
            assert actual[label]['indexes'][index_name]['order'] == expected
    if noop:
        assert before_raw == after_raw
    else:
        allowances.sort()
        cursor = 0
        changed = 0
        for offset, (old, new) in enumerate(zip(before_raw, after_raw)):
            if old == new:
                continue
            changed += 1
            while cursor < len(allowances) and allowances[cursor][1] <= offset:
                cursor += 1
            assert cursor < len(allowances) and allowances[cursor][0] <= offset, offset
        assert changed > 0
    return {'semantic_noop': noop, 'opaque_bytes_outside_allowed_spans_unchanged': True,
            'target_wire_hex': wire, 'all_typed_rows_and_logical_index_orders_match': True,
            'all_retained_key_row_entries_exact': True, 'full_child_maxima_and_original_topology': True,
            'affected_distinct_physical_indexes': physical_count, 'permitted_index_pages': page_count,
            'original_file_length_and_historical_counters_preserved': True}
