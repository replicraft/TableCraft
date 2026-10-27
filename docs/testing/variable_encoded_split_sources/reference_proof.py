"""Independent DATA/root/index/map proof; actual suffix order is observed."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals.wire import future_cell

PAGE = 4096
INDEXES = ('Ascending', 'Twin', 'Descending')


def normalized_index(index):
    return {key: value for key, value in index.items() if key != 'capacity'}


def compare(before, after, selected):
    old = Path(before['file']).read_bytes()
    new = Path(after['file']).read_bytes()
    assert len(new) > len(old) and len(new) % PAGE == 0
    assert set(before['maps']) == set(after['maps'])
    assert before['properties'] == after['properties']
    original_pages = len(old) // PAGE
    suffix = set(range(original_pages, len(new) // PAGE))
    allowed = bytearray(len(old))
    expected_bits = {}
    source = before['tables'][selected]['rows'][1]
    changed = after['tables'][selected]['rows'][1]
    target = changed['physical']
    root = changed['root']
    data_page = target['page']
    assert data_page in suffix and target['slot'] == 0
    assert (source['page'], source['slot']) == (changed['page'], changed['slot'])
    old_body = bytes.fromhex(source['body'])
    assert len(old_body) == root['end'] - root['start'] == 43
    assert old[root['start']:root['end']] == old_body
    pointer = ((data_page << 8) | target['slot']).to_bytes(4, 'little')
    assert new[root['start']:root['end']] == pointer + bytes(39)
    allowed[root['start']:root['end']] = bytes([1]) * 43
    word_offset = source['page'] * PAGE + 14 + source['slot'] * 2
    old_word = int.from_bytes(old[word_offset:word_offset + 2], 'little')
    assert old_word & 0xe000 == 0
    assert root['word'] == old_word | 0x4000
    allowed[word_offset:word_offset + 2] = bytes([1, 1])

    binary = selected == 'BinaryOne'
    fixed = 34 if binary else 30
    payload = bytes([255]) * 255 if binary else ('z' * 255).encode('utf-16le')
    end = fixed + len(payload)
    mask_bytes = 2 if binary else 1
    expected_record = (
        old_body[:fixed] + payload + end.to_bytes(2, 'little')
        + fixed.to_bytes(2, 'little') + bytes([1, 0]) + old_body[-mask_bytes:]
    )
    assert len(expected_record) == (297 if binary else 547)
    assert bytes.fromhex(changed['body']) == expected_record
    expected_page = bytearray(PAGE)
    expected_page[:2] = bytes([1, 1])
    expected_page[2:4] = (4082 - len(expected_record) - 2).to_bytes(2, 'little')
    definition = before['tables'][selected]['definition']
    expected_page[4:8] = definition.to_bytes(4, 'little')
    expected_page[12:14] = bytes([1, 0])
    expected_page[14:16] = (0x8000 | (PAGE - len(expected_record))).to_bytes(2, 'little')
    expected_page[-len(expected_record):] = expected_record
    assert new[data_page * PAGE:(data_page + 1) * PAGE] == expected_page

    index_pages = set()
    index_additions = {}
    purposes = {data_page: {'kind': 'DATA', 'definition': definition, 'hidden_slot': 0}}
    canonical = set()
    for label in INDEXES:
        a = before['tables'][selected]['indexes'][label]
        b = after['tables'][selected]['indexes'][label]
        assert (a['physical'], a['root']) == (b['physical'], b['root'])
        if a['physical'] in canonical:
            continue
        canonical.add(a['physical'])
        assert a['height'] == 0 and b['height'] > 0
        assert set(a['pages']) <= set(b['pages'])
        added = set(b['pages']) - set(a['pages'])
        assert added and added <= suffix
        assert not index_pages.intersection(added) and data_page not in added
        index_pages.update(added)
        index_additions[a['physical']] = added
        for page in a['pages']:
            allowed[page * PAGE:(page + 1) * PAGE] = bytes([1]) * PAGE
        for page in added:
            raw_page = new[page * PAGE:(page + 1) * PAGE]
            assert raw_page[:2] in (bytes([3, 1]), bytes([4, 1]))
            assert int.from_bytes(raw_page[4:8], 'little') == definition
            purposes[page] = {'kind': 'INDEX', 'physical': a['physical'], 'label': label, 'tag': raw_page[0]}
    assert suffix == index_pages | {data_page}
    assert set(purposes) == suffix

    old_hint = set(before['maps'][selected + ':Hint']['members'])
    assert len(old_hint) == 1
    failed = next(iter(old_hint))
    assert int.from_bytes(old[failed * PAGE + 2:failed * PAGE + 4], 'little') == 32
    assert len(expected_record) + 2 > 32
    index_roles = {selected + ':Index:' + label for label in INDEXES}
    for role, a in before['maps'].items():
        b = after['maps'][role]
        for field in ('table_page', 'row_start', 'row_end'):
            assert a[field] == b[field]
        for field in ('payload_type', 'map_slot', 'bitmap_pages'):
            assert a['decoded'][field] == b['decoded'][field]
        removed = set(a['members']) - set(b['members'])
        added = set(b['members']) - set(a['members'])
        if role == 'GlobalFree':
            assert removed == suffix and not added
        elif role == selected + ':Owned':
            assert added == {data_page} and not removed
        elif role == selected + ':Hint':
            assert removed == old_hint and added == {data_page}
        elif role in index_roles:
            label = role.split(':Index:', 1)[1]
            physical = before['tables'][selected]['indexes'][label]['physical']
            assert added == index_additions[physical] and not removed
        else:
            assert a == b
            continue
        for page, set_bit in [(page, False) for page in sorted(removed)] + [(page, True) for page in sorted(added)]:
            cell = future_cell(old, a, page)
            assert cell['set'] != set_bit
            offset, mask = cell['offset'], cell['mask']
            original_byte = expected_bits.setdefault(offset, old[offset])
            expected_bits[offset] = original_byte | mask if set_bit else original_byte & ~mask
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
            if name == selected and identity == 1:
                assert row['fixed'] == current['fixed']
                expected_values = dict(row['values'])
                expected_values['KeyValue'] = ('Binary:' if binary else 'Text:') + payload.hex()
                assert current['values'] == expected_values
            else:
                assert row == current
        for label, index in table['indexes'].items():
            if name != selected or label == 'PrimaryID':
                assert normalized_index(index) == normalized_index(other['indexes'][label])
    for offset, (left, right) in enumerate(zip(old, new[:len(old)], strict=True)):
        assert allowed[offset] or left == right, f'Unqualified original byte change at {offset}'
    return {
        'selected': selected,
        'added_data_page': data_page,
        'added_index_pages': sorted(index_pages),
        'suffix_purposes_in_actual_order': [{'page': page, **purposes[page]} for page in sorted(purposes)],
        'root_allocation_bytes': 43,
        'root_remainder_zeroed': True,
        'new_hidden_record_bytes': len(expected_record),
        'old_root_and_index_ROW_identity_retained': True,
        'all_unselected_row_bodies_and_fields_preserved': True,
        'all_unaffected_index_and_metadata_bytes_preserved': True,
        'ordinary_owned_added_exact_DATA_only': True,
        'global_clears_equal_complete_suffix': True,
        'map_shapes_and_bitmap_storage_unchanged': True,
        'exact_map_byte_edits': len(expected_bits),
        'failed_highest_hint': failed,
    }
