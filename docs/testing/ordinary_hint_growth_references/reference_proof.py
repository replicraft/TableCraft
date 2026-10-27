"""Independent DATA/root/index/map proof; actual suffix order is observed."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
import hashlib

from ordinary_hint_growth_references import map_proof

PAGE = 4096
INDEXES = ('Ascending', 'Twin', 'Descending')


def normalized_index(index):
    return {key: value for key, value in index.items() if key != 'capacity'}


def compare(before, after, selected):
    old = Path(before['file']).read_bytes()
    new = Path(after['file']).read_bytes()
    assert hashlib.sha256(old).hexdigest() == before['sha256']
    assert hashlib.sha256(new).hexdigest() == after['sha256']
    return compare_bytes(before, after, selected, old, new)


def compare_bytes(before, after, selected, old, new):
    assert len(new) > len(old) and len(new) % PAGE == 0
    assert set(before['maps']) == set(after['maps'])
    assert before['properties'] == after['properties']
    original_pages = len(old) // PAGE
    suffix = set(range(original_pages, len(new) // PAGE))
    allowed = bytearray(len(old))
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

    old_hint = set(before['maps'][selected + ':Hint']['members'])
    assert old_hint
    failed = max(old_hint)
    assert int.from_bytes(old[failed * PAGE + 2:failed * PAGE + 4], 'little') == 32
    assert len(expected_record) + 2 > 32
    index_roles = {selected + ':Index:' + label for label in INDEXES}
    transitions = {}
    metadata_pages = set()
    for role, original_map in before['maps'].items():
        current_map = after['maps'][role]
        if role == 'GlobalFree':
            removed, added = suffix, set()
        elif role == selected + ':Owned':
            removed, added = set(), {data_page}
        elif role == selected + ':Hint':
            removed, added = {failed}, {data_page}
        elif role in index_roles:
            label = role.split(':Index:', 1)[1]
            physical = before['tables'][selected]['indexes'][label]['physical']
            removed, added = set(), index_additions[physical]
        else:
            assert original_map == current_map
            continue
        transition = map_proof.compare(
            old, new, original_map, current_map, removed, added, allowed,
        )
        transitions[role] = transition
        for bitmap in transition['fresh_bitmap_pages']:
            identity = (role, original_map['table_page'], original_map['row_start'])
            if bitmap in purposes:
                assert purposes[bitmap]['kind'] == 'MAP_BITMAP'
                assert purposes[bitmap]['row_identity'] == list(identity[1:])
                continue
            assert bitmap not in index_pages and bitmap != data_page
            metadata_pages.add(bitmap)
            purposes[bitmap] = {
                'kind': 'MAP_BITMAP',
                'role': role,
                'row_identity': list(identity[1:]),
                'membership': 'GlobalFree allocation only',
            }
    assert suffix == index_pages | metadata_pages | {data_page}
    assert set(purposes) == suffix
    for bitmap in metadata_pages:
        assert all(
            bitmap not in after['maps'][role]['members']
            for role in after['maps']
        )

    counters = []
    for name, table in before['tables'].items():
        definition_offset = table['definition'] * PAGE
        assert old[definition_offset + 16:definition_offset + 20] == new[definition_offset + 16:definition_offset + 20]
        physical_count = int.from_bytes(old[definition_offset + 51:definition_offset + 55], 'little')
        for physical in range(physical_count):
            offset = definition_offset + 63 + 12 * physical
            original_block = old[offset:offset + 12]
            current_block = new[offset:offset + 12]
            counters.append({
                'table': name,
                'physical': physical,
                'block_offset': offset,
                'before_hex': original_block.hex(),
                'after_hex': current_block.hex(),
                'historical_offset': offset + 4,
                'historical_before': int.from_bytes(original_block[4:8], 'little'),
                'historical_after': int.from_bytes(current_block[4:8], 'little'),
            })
            assert original_block == current_block, counters[-1]

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
        'map_transitions': transitions,
        'fresh_map_bitmap_pages': sorted(metadata_pages),
        'physical_counter_blocks': counters,
        'all_observed_counter_fields_unchanged': True,
        'failed_highest_hint': failed,
    }
