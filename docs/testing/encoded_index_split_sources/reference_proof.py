"""Independent exact source-prefix and map-bit proof for six public references."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals.wire import future_cell

PAGE = 4096
INDEXES = ('Ascending', 'Twin', 'Descending')


def compare(before, after, selected):
    old = Path(before['file']).read_bytes()
    new = Path(after['file']).read_bytes()
    assert len(new) >= len(old) and len(new) % PAGE == 0
    assert set(before['maps']) == set(after['maps'])
    assert before['properties'] == after['properties']
    allowed = bytearray(len(old))
    expected_bits = {}
    before_page_count = len(old) // PAGE
    selected_rows = before['tables'][selected]['rows']
    changed = selected_rows[1]['selected']
    actual = after['tables'][selected]['rows'][1]['selected']
    assert (changed['offset'], changed['length']) == (actual['offset'], actual['length'])
    allowed[changed['offset']:changed['offset'] + changed['length']] = bytes([1]) * changed['length']
    added_pages = set()
    canonical = []
    for label in INDEXES:
        a = before['tables'][selected]['indexes'][label]
        b = after['tables'][selected]['indexes'][label]
        assert (a['physical'], a['root']) == (b['physical'], b['root'])
        if a['physical'] in canonical:
            continue
        canonical.append(a['physical'])
        assert a['height'] == 0 and b['height'] > 0
        assert set(a['pages']) <= set(b['pages'])
        added = set(b['pages']) - set(a['pages'])
        assert added and all(page >= before_page_count for page in added)
        assert not added_pages.intersection(added)
        added_pages.update(added)
        for page in a['pages']:
            allowed[page * PAGE:(page + 1) * PAGE] = bytes([1]) * PAGE
    assert added_pages == set(range(before_page_count, len(new) // PAGE))

    for role, a in before['maps'].items():
        b = after['maps'][role]
        for field in ('table_page', 'row_start', 'row_end'):
            assert a[field] == b[field]
        for field in ('payload_type', 'map_slot', 'bitmap_pages'):
            assert a['decoded'][field] == b['decoded'][field]
        is_index = role in {f'{selected}:Index:{label}' for label in INDEXES}
        if not is_index and role != 'GlobalFree':
            assert a == b
            continue
        removed = set(a['members']) - set(b['members'])
        added = set(b['members']) - set(a['members'])
        if role == 'GlobalFree':
            assert not added and removed == added_pages
            changes = removed
            set_bit = False
        else:
            assert not removed
            label = role.split(':Index:', 1)[1]
            expected = set(after['tables'][selected]['indexes'][label]['pages'])
            expected -= set(before['tables'][selected]['indexes'][label]['pages'])
            assert added == expected
            changes = added
            set_bit = True
        for page in sorted(changes):
            cell = future_cell(old, a, page)
            assert cell['set'] != set_bit
            offset, mask = cell['offset'], cell['mask']
            byte = expected_bits.setdefault(offset, old[offset])
            expected_bits[offset] = byte | mask if set_bit else byte & ~mask
    for offset, expected in expected_bits.items():
        assert new[offset] == expected
        allowed[offset] = 1

    for name, table in before['tables'].items():
        other = after['tables'][name]
        assert table['definition'] == other['definition']
        assert table['count'] == other['count']
        assert table['columns'] == other['columns']
        for identity, row in table['rows'].items():
            current = other['rows'][identity]
            assert row['page'] == current['page'] and row['slot'] == current['slot']
            if name != selected or identity != 1:
                assert row == current
            else:
                left = bytes.fromhex(row['body'])
                right = bytes.fromhex(current['body'])
                # Full source-prefix comparison below independently protects
                # the footer, masks, fixed fields and all other row bytes.
                assert len(left) == len(right)
                assert row['fixed'] == current['fixed']
        if name != selected:
            original_indexes = {
                label: {key: value for key, value in index.items() if key != 'capacity'}
                for label, index in table['indexes'].items()
            }
            actual_indexes = {
                label: {key: value for key, value in index.items() if key != 'capacity'}
                for label, index in other['indexes'].items()
            }
            assert original_indexes == actual_indexes
        else:
            assert table['indexes']['PrimaryID'] == other['indexes']['PrimaryID']
    for position, (left, right) in enumerate(zip(old, new[:len(old)], strict=True)):
        assert allowed[position] or left == right, f'Unqualified original byte change at {position}'
    return {
        'selected': selected,
        'added_index_pages': sorted(added_pages),
        'original_bytes_preserved_outside_exact_field_index_and_map_bits': True,
        'all_original_row_references_preserved': True,
        'ordinary_table_and_lval_maps_unchanged': True,
        'primary_and_sibling_indexes_unchanged': True,
        'selected_table_definition_and_all_counters_unchanged': True,
        'global_allocation_equals_new_index_pages': True,
        'exact_map_byte_edits': len(expected_bits),
    }
