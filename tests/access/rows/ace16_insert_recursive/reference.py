"""Public recursive INSERT observations from actual source bytes, never candidate goldens."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_root_split import reference as root_reference
from ace16_insert_candidates import model as record_codec

PAGE = 4096
ENTRY_START = 480
MAX_ID = (1 << 31) - 1
DELETED = 0x8000
OFFSET = 0x1fff
PREVIOUS = 12
NEXT = 16



def put(raw, at, value, width):
    raw[at:at + width] = value.to_bytes(width, 'little')


def data_patch(before, after, table_before, table_after, layout):
    row = table_after['rows'][MAX_ID]
    number, slot = row['page'], row['slot']
    at = number * PAGE
    actual = after[at:at + PAGE]
    body = root_reference.record(MAX_ID, b'')
    if number >= len(before) // PAGE:
        expected = bytearray(PAGE)
        expected[:2] = bytes([1, 1])
        put(expected, 2, PAGE - 14 - 2 - len(body), 2)
        put(expected, 4, layout['definition'], 4)
        put(expected, 12, 1, 2)
        put(expected, 14, PAGE - len(body), 2)
        expected[PAGE - len(body):] = body
        assert slot == 0 and actual == expected
        return number, 'fresh DATA page', {number}
    original = before[at:at + PAGE]
    expected = bytearray(original)
    count = int.from_bytes(original[12:14], 'little')
    assert number in {item['page'] for item in table_before['rows'].values()} or original[:2] == bytes([1, 1])
    if slot == count:
        assert count < 255
        end = PAGE if count == 0 else int.from_bytes(original[12 + 2 * count:14 + 2 * count], 'little') & OFFSET
        start = end - len(body)
        assert start >= 14 + 2 * (count + 1)
        put(expected, 2, start - 14 - 2 * (count + 1), 2)
        put(expected, 12, count + 1, 2)
        put(expected, 14 + 2 * slot, start, 2)
        policy = 'append ordinary slot on existing DATA page'
    else:
        assert slot < count
        word = int.from_bytes(original[14 + 2 * slot:16 + 2 * slot], 'little')
        assert word & DELETED
        start = word & OFFSET
        end = PAGE if slot == 0 else int.from_bytes(original[12 + 2 * slot:14 + 2 * slot], 'little') & OFFSET
        assert end - start == len(body)
        put(expected, 14 + 2 * slot, start, 2)
        policy = 'reuse deleted ordinary slot'
    expected[start:start + len(body)] = body
    assert actual == expected, ('Public DATA page differs from closed append/reuse framing', number)
    return number, policy, set()


def compare(case, before_state, after_state):
    before, table, layout, maps, nodes, leaves = before_state
    after, current, current_layout, current_maps, current_nodes, current_leaves = after_state
    assert len(after) >= len(before) and len(after) % PAGE == 0
    assert set(current['rows']) == set(table['rows']) | {MAX_ID}
    assert MAX_ID not in table['rows']
    for identity, row in table['rows'].items():
        retained = current['rows'][identity]
        for name in ('page', 'slot', 'values'):
            assert retained[name] == row[name], (identity, name)
        assert current['raw_rows'][identity] == table['raw_rows'][identity]
    for name in ('definition', 'root', 'owned', 'hint', 'index_owned', 'global', 'table_count', 'history'):
        assert current_layout[name] == layout[name], name
    assert current['count'] == table['count'] + 1
    assert current_layout['historical_before'] == layout['historical_before'] + 1
    root, parent, target = case['root'], case['parent'], case['leaf']
    assert root == layout['root']
    assert set(nodes) <= set(current_nodes)
    fresh_index = set(current_nodes) - set(nodes)
    assert len(fresh_index) == (3 if case['mode'] == 'root' else 2)
    assert current_nodes[root]['height'] == (2 if case['mode'] == 'root' else nodes[root]['height'])
    new_leaves = set(current_leaves) - set(leaves)
    assert len(new_leaves) == 1
    new_left = next(iter(new_leaves))
    position = leaves.index(target)
    assert current_leaves == leaves[:position] + [new_left] + leaves[position:]
    changed = nodes[target]['entries'] + [record_codec.int_key(
        MAX_ID, (current['rows'][MAX_ID]['page'] << 8) | current['rows'][MAX_ID]['slot'])]
    # Jackcess 5.0.4 moves ceil(entries / 2) to fresh LEFT.
    # Engineering floor-halving is deliberately not the public oracle.
    middle = (len(changed) + 1) // 2
    assert current_nodes[new_left]['entries'] == changed[:middle]
    assert current_nodes[target]['entries'] == changed[middle:]
    for number in leaves:
        if number != target:
            assert current_nodes[number]['entries'] == nodes[number]['entries']
    def children(node):
        return node['children'] + ([node['tail']] if node['tail'] else [])

    original_children = children(nodes[parent])
    assert original_children[-1] == target and nodes[parent]['tail'] == target
    final_children = original_children[:-1] + [new_left, target]
    # The new leaf bound adds one stored parent entry; its original tail stays
    # a tail during the split. Public splitDataPage halves stored entries.
    parent_middle = (len(nodes[parent]['entries']) + 2) // 2
    internal_fresh = fresh_index - {new_left}
    if case['mode'] == 'root':
        nested = children(current_nodes[root])
        assert len(nested) == 2 and set(nested) == internal_fresh
        parent_left, parent_right = nested
    else:
        assert len(internal_fresh) == 1
        parent_left = next(iter(internal_fresh))
        parent_right = parent
        original_root_children = children(nodes[root])
        position = original_root_children.index(parent)
        assert children(current_nodes[root]) == (
            original_root_children[:position] + [parent_left, parent]
            + original_root_children[position + 1:])
    assert current_nodes[parent_left]['height'] == current_nodes[parent_right]['height'] == 1
    assert children(current_nodes[parent_left]) == final_children[:parent_middle]
    assert children(current_nodes[parent_right]) == final_children[parent_middle:]
    assert parent_middle > 0 and parent_middle < len(final_children)
    data_page, data_policy, new_data = data_patch(before, after, table, current, layout)
    assert data_page in current_maps['owned']['members']
    eof_before, eof_after = len(before) // PAGE, len(after) // PAGE
    suffix = set(range(eof_before, eof_after))
    assert fresh_index | new_data == suffix and not fresh_index & new_data
    assert all(number >= eof_before for number in fresh_index)
    assert current_maps['global']['members'] == maps['global']['members'] - suffix
    assert current_maps['owned']['members'] == maps['owned']['members'] | new_data
    assert current_maps['index_owned']['members'] == maps['index_owned']['members'] | fresh_index
    added_hint = current_maps['hint']['members'] - maps['hint']['members']
    removed_hint = maps['hint']['members'] - current_maps['hint']['members']
    assert added_hint <= {data_page}
    for number in removed_hint:
        source = after if number == data_page else before
        at = number * PAGE
        assert root_reference.number(source, at + 12, 2) == 255 or root_reference.number(source, at + 2, 2) < 15
    expected = bytearray(before)
    for offset, value in ((layout['table_count'], current['count']),
                          (layout['history'], current_layout['historical_before'])):
        put(expected, offset, value, 4)
    map_changes = {}
    for role, old in maps.items():
        new = current_maps[role]
        assert new['kind'] == old['kind'] and new['range'] == old['range']
        assert new['locations'] == old['locations']
        transitions = []
        for number in sorted(old['members'] ^ new['members']):
            absolute, mask = old['locations'][number]
            if number in new['members']:
                expected[absolute] |= mask
            else:
                expected[absolute] &= ~mask
            transitions.append({'page': number, 'offset': absolute, 'mask': mask,
                                'set': number in new['members']})
        map_changes[role] = transitions
    affected = {root, parent, target}
    linked = set()
    for number in (target, parent):
        previous = nodes[number]['previous']
        if previous:
            linked.add(previous)
    for number in affected:
        expected[number * PAGE:(number + 1) * PAGE] = after[number * PAGE:(number + 1) * PAGE]
    peer_scratch = []
    for number in linked - affected:
        at = number * PAGE
        original_page = before[at:at + PAGE]
        public_page = after[at:at + PAGE]
        active_end = ENTRY_START + (PAGE - ENTRY_START - nodes[number]['free'])
        # Relinking a public dirty page can serialize inactive shared-buffer
        # scratch. Qualify its complete immutable frame before observing that
        # suffix; this exception grants no active payload or header changes.
        assert original_page[:NEXT] == public_page[:NEXT]
        assert original_page[NEXT + 4:active_end] == public_page[NEXT + 4:active_end]
        assert current_nodes[number]['entries'] == nodes[number]['entries']
        expected[at + NEXT:at + NEXT + 4] = public_page[NEXT:NEXT + 4]
        expected[at + active_end:at + PAGE] = public_page[active_end:]
        changed = sum(old != new for old, new in
                      zip(original_page[active_end:], public_page[active_end:], strict=True))
        peer_scratch.append({'page': number, 'inactive_start': active_end,
                             'inactive_end': PAGE, 'changed_bytes': changed,
                             'header_bitmap_active_unchanged_except_next': True})
    if data_page < eof_before:
        expected[data_page * PAGE:(data_page + 1) * PAGE] = after[data_page * PAGE:(data_page + 1) * PAGE]
    assert len(expected) == len(before)
    if expected != after[:len(before)]:
        position = next(index for index, pair in enumerate(zip(expected, after, strict=False))
                        if pair[0] != pair[1])
        raise AssertionError(('Unrelated original prefix byte changed', position,
                              expected[position], after[position]))
    return {
        'before_rows': table['count'], 'after_rows': current['count'],
        'before_history': layout['historical_before'], 'after_history': current_layout['historical_before'],
        'new_row': {'id': MAX_ID, 'page': data_page, 'slot': current['rows'][MAX_ID]['slot'],
                    'placement': data_policy},
        'root_before_height': nodes[root]['height'], 'root_after_height': current_nodes[root]['height'],
        'suffix': [{'page': number, 'purpose': 'INDEX' if number in fresh_index else 'DATA'}
                   for number in sorted(suffix)],
        'retained_target': target, 'new_left': new_left,
        'leaf_counts': [len(current_nodes[new_left]['entries']), len(current_nodes[target]['entries'])],
        'public_parent_partition': {
            'left': parent_left, 'right': parent_right,
            'left_children': final_children[:parent_middle],
            'right_children': final_children[parent_middle:],
        },
        'public_split_rounding': 'Ceiling entry count for fresh LEFT, from pinned Jackcess source.',
        'changed_original_index_pages': sorted(affected), 'repaired_previous_next': sorted(linked - affected),
        'map_changes': map_changes,
        'public_linked_peer_scratch': peer_scratch,
        'index_frames': {str(number): {'height': current_nodes[number]['height'],
                                      'prefix': current_nodes[number]['prefix'],
                                      'free': current_nodes[number]['free']}
                         for number in sorted(affected | fresh_index)},
        'candidate_policy': 'Not used. Engineering images derive source/request ROWs independently.',
    }
