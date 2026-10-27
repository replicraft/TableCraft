"""Separate read-only height-two/non-root-parent and sparse-history certificate."""
import hashlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals import wire


def certify(raw, tables, selected, sparse):
    proof = wire.certify(raw, tables, selected, sparse)
    assert selected['ROOT_HEIGHT'] == '2'
    assert selected['PARENT_IS_ROOT'] == 'false'
    root = proof['root']
    parent = proof['parent']
    assert root != parent and proof['root_height'] == 2
    owner = tables['SparseInt32']['owner']
    nodes, leaves, entries = wire.tree(raw, root, owner)
    root_node = nodes[root]
    root_children = list(root_node['children'])
    if root_node['tail']:
        root_children.append(root_node['tail'])
    assert root_children[-1] == parent
    assert nodes[parent]['height'] == 1
    parent_children = list(nodes[parent]['children'])
    if nodes[parent]['tail']:
        parent_children.append(nodes[parent]['tail'])
    assert parent_children[-1] == proof['leaf'] == leaves[-1]
    levels = {}
    for height in (0, 1, 2):
        level = [number for number, node in nodes.items() if node['height'] == height]
        assert level
        levels[str(height)] = level
        for position, number in enumerate(level):
            previous = level[position - 1] if position else 0
            following = level[position + 1] if position + 1 < len(level) else 0
            assert nodes[number]['previous'] == previous
            assert nodes[number]['next'] == following
    internal = {}
    for number, node in nodes.items():
        if node['height']:
            internal[str(number)] = {
                'height': node['height'], 'children': node['children'],
                'tail': node['tail'], 'full_bounds': [entry.hex() for entry in node['entries']],
                'previous': node['previous'], 'next': node['next'],
            }
    proof['internal_parent_is_not_root'] = True
    proof['ordered_sibling_levels'] = levels
    proof['original_internal_structure'] = internal
    proof['full_stream_sha256'] = hashlib.sha256(b''.join(entries)).hexdigest()
    retained = {}
    for value in selected['keep']:
        row = tables['SparseInt32']['rows'][value]
        retained[str(value)] = {key: row[key] for key in ('marker', 'page', 'slot')}
    proof['retained_row_identities'] = retained
    proof['sentinel_rows'] = tables['Sentinel']['rows']
    proof['target_leaf_stream_sha256'] = hashlib.sha256(
        b''.join(nodes[proof['leaf']]['entries']),
    ).hexdigest()
    return proof


def compare_history(before, after):
    result = wire.compare_history(before, after)
    assert before['internal_parent_is_not_root'] and after['internal_parent_is_not_root']
    assert before['ordered_sibling_levels'] == after['ordered_sibling_levels']
    assert before['original_internal_structure'] == after['original_internal_structure']
    assert before['retained_row_identities'] == after['retained_row_identities']
    assert before['sentinel_rows'] == after['sentinel_rows']
    assert before['target_leaf_stream_sha256'] == after['target_leaf_stream_sha256']
    result['all_three_sibling_levels_preserved'] = True
    result['every_internal_full_bound_and_child_preserved'] = True
    return result
