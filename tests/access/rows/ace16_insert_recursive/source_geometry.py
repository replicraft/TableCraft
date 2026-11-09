"""Pure sparse-history certificate; no process calls or database writes."""
import hashlib
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals import wire

PAGE_BYTES = 4096
MAX_TARGET_ROWS = 4096
CAPACITY = 3616
INT32_MAX = (1 << 31) - 1


def selection(path):
    scalar = {}
    keep = set()
    leaves = {}
    for line in Path(path).read_text().splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'KEEP':
            assert len(fields) == 2
            value = int(fields[1])
            assert value not in keep
            keep.add(value)
        elif tag == 'ORIGINAL_LEAF':
            assert len(fields) == 4
            number, count, maximum = map(int, fields[1:])
            assert number not in leaves
            leaves[number] = (count, maximum)
        else:
            assert len(fields) == 2 and tag not in scalar
            scalar[tag] = fields[1]
    assert set(scalar) == {
        'SOURCE_SHA256', 'MODE', 'INITIAL_ROWS', 'GOAL_MET', 'ROOT',
        'PARENT', 'LEAF', 'SELECTED', 'LEAF_BEST_AFTER', 'PARENT_BEST_AFTER',
        'LEFT_COUNT', 'TARGET_COUNT',
    }
    assert scalar['MODE'] in ('root', 'internal')
    assert scalar['GOAL_MET'] == 'true'
    assert 0 < len(keep) <= MAX_TARGET_ROWS and INT32_MAX not in keep
    return scalar, keep, leaves


def certify(original, sparse, selection_path, owner):
    """Require retained graph/ROWs and actual INSERT pressure, not UPDATE pressure."""
    facts, keep, recorded_leaves = selection(selection_path)
    assert hashlib.sha256(original).hexdigest() == facts['SOURCE_SHA256']
    assert len(original) % PAGE_BYTES == 0 and len(sparse) % PAGE_BYTES == 0
    root = int(facts['ROOT'])
    old, old_leaves, old_entries = wire.tree(original, root, owner)
    new, new_leaves, new_entries = wire.tree(sparse, root, owner)
    assert [wire.key(entry) for entry in old_entries] == list(range(1, int(facts['INITIAL_ROWS']) + 1))
    assert set(new) == set(old) and new_leaves == old_leaves
    assert [wire.key(entry) for entry in new_entries] == sorted(keep)
    assert new_entries == [entry for entry in old_entries if wire.key(entry) in keep]
    assert recorded_leaves == {
        number: (len(old[number]['entries']), wire.key(old[number]['entries'][-1]))
        for number in old_leaves
    }
    for number, previous in old.items():
        current = new[number]
        for name in ('height', 'previous', 'next', 'tail', 'children'):
            assert current[name] == previous[name], (number, name)
        if previous['height']:
            assert current['entries'] == previous['entries'], number
    target = new[int(facts['LEAF'])]
    parent = new[int(facts['PARENT'])]
    height = 1 if facts['MODE'] == 'root' else 2
    assert new[root]['height'] == height and parent['height'] == 1
    assert (parent['number'] == root) == (facts['MODE'] == 'root')
    assert parent['tail'] == target['number'] != 0
    assert len(target['entries']) == int(facts['TARGET_COUNT'])
    assert wire.key(target['entries'][-1]) == int(facts['SELECTED'])
    changed = target['entries'] + [wire.encoded(INT32_MAX, bytes(4))]
    middle = len(changed) // 2
    leaf_bytes = wire.best_size(changed, False)
    assert leaf_bytes == int(facts['LEAF_BEST_AFTER']) > CAPACITY
    assert wire.best_size(changed[:middle], False) <= CAPACITY
    assert wire.best_size(changed[middle:], False) <= CAPACITY
    bounds = parent['entries'] + [changed[middle - 1]]
    parent_bytes = wire.best_size(bounds, True)
    assert parent_bytes == int(facts['PARENT_BEST_AFTER']) > CAPACITY
    assert middle == int(facts['LEFT_COUNT'])
    return {
        'mode': facts['MODE'], 'generated_rows': len(old_entries),
        'live_rows': len(new_entries), 'retained_index_pages': len(new),
        'root': root, 'root_height': height, 'parent': parent['number'],
        'leaf': target['number'], 'leaf_insert_bytes': leaf_bytes,
        'parent_insert_bytes': parent_bytes,
        'historical_counter': 'Not inferred from live count; capture from original physical metadata.',
        'data_placement': 'Not predicted from public output; future engineering model binds source and requests.',
    }
