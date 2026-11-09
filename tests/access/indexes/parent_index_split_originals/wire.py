"""Independent native Int32 geometry and sparse-history gate; no process calls."""
import hashlib
import json
from pathlib import Path

PAGE_BYTES = 4096
ENTRY_START = 480
MASK_START = 27
CAPACITY = PAGE_BYTES - ENTRY_START
KEY_ROW_BYTES = 9
CHILD_BYTES = 4
LEAF = 4
NODE = 3
MAX_DEPTH = 16
MAX_IMAGE_BYTES = 64 * 1024 * 1024
MAX_LIVE_ROWS = 16_384
GENERATION_ROWS = 150_000
REFERENCE_BYTES = 4
ROW_START = 14
OFFSET_MASK = 0x1FFF
ROW_COUNT_OFFSET = 12
MAX_SLOTS = 255
DELETED_MASK = 0x8000
COLUMN_COUNT = 2
COLUMN_COUNT_BYTES = 2
INT32_BYTES = 4
FIXED_RECORD_BYTES = COLUMN_COUNT_BYTES + COLUMN_COUNT * INT32_BYTES + 1
PRESENT_MASK = (1 << COLUMN_COUNT) - 1
FORMAT_OFFSET = 20
SUBVERSION_OFFSET = 21
RESERVED_END = 24
ENCODING_KEY_START = 62
ENCODING_KEY_END = 66
MAP_INLINE = 0
MAP_INDIRECT = 1
MAP_BASE_OFFSET = 1
MAP_BITMAP_OFFSET = 5
BITMAP_PAGE_START = 4


def unsigned(data, offset, width=4, order='little'):
    return int.from_bytes(data[offset:offset + width], order)


def key(entry):
    assert len(entry) == KEY_ROW_BYTES and entry[0] == 0x7f
    value = unsigned(entry, 1, 4, 'big') ^ 0x80000000
    return value - (1 << 32) if value >= 1 << 31 else value


def encoded(value, reference):
    return b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big') + reference


def best_size(entries, node):
    if not entries:
        return 0
    prefix = KEY_ROW_BYTES
    for entry in entries:
        equal = 0
        while equal < prefix and entries[0][equal] == entry[equal]:
            equal += 1
        prefix = equal
    return len(entries) * (KEY_ROW_BYTES + (CHILD_BYTES if node else 0)) - prefix * (len(entries) - 1)


def page(raw, number, owner):
    body = raw[number * PAGE_BYTES:(number + 1) * PAGE_BYTES]
    assert len(body) == PAGE_BYTES
    kind = body[0]
    assert kind in (LEAF, NODE) and body[1] == 1
    assert unsigned(body, 4) == owner and body[8:12] == bytes(4)
    assert (kind == LEAF) == (body[26] == 0)
    prefix = unsigned(body, 24, 2)
    assert prefix <= KEY_ROW_BYTES
    entries = []
    children = []
    previous = 0
    first = None
    for end in range(1, CAPACITY + 1):
        if body[MASK_START + end // 8] & (1 << (end % 8)) == 0:
            continue
        suffix = body[ENTRY_START + previous:ENTRY_START + end]
        full = suffix if first is None else first[:prefix] + suffix
        if first is None:
            first = full
        assert len(full) == KEY_ROW_BYTES + (CHILD_BYTES if kind == NODE else 0)
        entry = full[:KEY_ROW_BYTES]
        key(entry)
        entries.append(entry)
        if kind == NODE:
            child = int.from_bytes(full[-CHILD_BYTES:], 'big')
            assert child > 0
            children.append(child)
        previous = end
    assert entries and unsigned(body, 2, 2) == CAPACITY - previous
    return {'number': number, 'height': body[26], 'prefix': prefix,
            'free': unsigned(body, 2, 2), 'previous': unsigned(body, 12),
            'next': unsigned(body, 16), 'tail': unsigned(body, 20),
            'entries': entries, 'children': children,
            'sha256': hashlib.sha256(body).hexdigest()}


def tree(raw, root, owner):
    nodes = {}
    leaves = []

    def walk(number, depth):
        assert depth < MAX_DEPTH and number not in nodes
        current = page(raw, number, owner)
        nodes[number] = current
        if current['height'] == 0:
            leaves.append(number)
            current['subtree_maximum'] = current['entries'][-1].hex()
            return current['entries']
        result = []
        for bound, child in zip(current['entries'], current['children'], strict=True):
            nested = walk(child, depth + 1)
            assert nested[-1] == bound and nodes[child]['height'] + 1 == current['height']
            result.extend(nested)
        if current['tail']:
            nested = walk(current['tail'], depth + 1)
            assert nodes[current['tail']]['height'] + 1 == current['height']
            result.extend(nested)
        current['subtree_maximum'] = result[-1].hex()
        return result

    entries = walk(root, 0)
    assert entries == sorted(entries)
    for position, number in enumerate(leaves):
        current = nodes[number]
        expected = (leaves[position - 1] if position else 0,
                    leaves[position + 1] if position + 1 < len(leaves) else 0)
        assert (current['previous'], current['next']) == expected
    for height in set(current['height'] for current in nodes.values()):
        level = [current for current in nodes.values() if current['height'] == height]
        for position, current in enumerate(level):
            expected = (level[position - 1]['number'] if position else 0,
                        level[position + 1]['number'] if position + 1 < len(level) else 0)
            assert (current['previous'], current['next']) == expected
    return nodes, leaves, entries


def parse_description(text):
    tables = {}
    map_lines = []
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag in ('FILE', 'MAP', 'MEMBER'):
            map_lines.append(line)
        elif tag == 'TABLE':
            tables[fields[1]] = {'rows': {}, 'columns': [], 'indexes': {},
                                 'count': int(fields[2]), 'owner': int(fields[3])}
        elif tag == 'COLUMN':
            tables[fields[1]]['columns'].append(fields[2:])
        elif tag == 'ROW':
            ordinal, value, marker, number, slot = map(int, fields[2:])
            table = tables[fields[1]]
            assert value not in table['rows'] and ordinal == len(table['rows'])
            table['rows'][value] = {'ordinal': ordinal, 'marker': marker,
                                    'page': number, 'slot': slot}
        elif tag == 'INDEX':
            tables[fields[1]]['indexes'][fields[2]] = {
                'physical': int(fields[3]), 'root': int(fields[4]),
                'historical': int(fields[5]), 'historical_offset': int(fields[6]),
                'primary': fields[7], 'required': fields[8], 'unique': fields[9],
                'ignore_nulls': fields[10], 'columns': [], 'order': [],
            }
        elif tag == 'INDEX_COLUMN':
            tables[fields[1]]['indexes'][fields[2]]['columns'].append(fields[3:])
        elif tag == 'ENTRY':
            tables[fields[1]]['indexes'][fields[2]]['order'].append(int(fields[3]))
        else:
            raise AssertionError('Unexpected public getter tag: ' + tag)
    assert set(tables) == {'SparseInt32', 'Sentinel'}
    return tables, '\n'.join(map_lines) + '\n'


def selection(text):
    result = {'keep': [], 'leaves': []}
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'KEEP':
            result['keep'].append(int(fields[1]))
        elif fields[0] == 'ORIGINAL_LEAF':
            result['leaves'].append(list(map(int, fields[1:])))
        else:
            assert fields[0] not in result
            result[fields[0]] = fields[1]
    assert len(result['keep']) == len(set(result['keep']))
    return result


def certify(raw, tables, selected, sparse):
    assert len(raw) <= MAX_IMAGE_BYTES and len(raw) % PAGE_BYTES == 0
    assert raw[:4] == b'\x00\x01\x00\x00' and raw[4:20] == b'Standard ACE DB\x00'
    assert raw[FORMAT_OFFSET] in (5, 6) and raw[SUBVERSION_OFFSET:RESERVED_END] == bytes(3)
    assert raw[ENCODING_KEY_START:ENCODING_KEY_END] == bytes.fromhex('fb8abc4e')
    target = tables['SparseInt32']
    assert target['columns'] == [['Key', 'LONG', '0', '4'], ['Marker', 'LONG', '1', '4']]
    assert set(target['indexes']) == {'PrimaryKey'}
    index = target['indexes']['PrimaryKey']
    assert (index['primary'], index['required'], index['unique'], index['ignore_nulls']) == ('true', 'true', 'true', 'false')
    assert index['columns'] == [['Key', 'true']]
    assert target['count'] == len(target['rows'])
    assert target['count'] <= (MAX_LIVE_ROWS if sparse else GENERATION_ROWS)
    for value, row in target['rows'].items():
        assert row['marker'] == value * 3
    assert tables['Sentinel']['count'] == 1
    assert tables['Sentinel']['rows'][91]['marker'] == 12345
    root = int(selected['ROOT'])
    assert root == index['root']
    nodes, leaves, entries = tree(raw, root, target['owner'])
    values = [key(entry) for entry in entries]
    assert values == index['order'] == sorted(target['rows'])
    for entry in entries:
        row = target['rows'][key(entry)]
        reference = (row['page'] << 8) | row['slot']
        assert entry[-REFERENCE_BYTES:] == reference.to_bytes(REFERENCE_BYTES, 'big')
    leaf_number = int(selected['LEAF'])
    parent_number = int(selected['PARENT'])
    leaf = nodes[leaf_number]
    parent = nodes[parent_number]
    assert leaf_number == leaves[-1] and parent['height'] == 1
    assert leaf['height'] == 0 and len(leaf['entries']) == int(selected['TARGET_COUNT'])
    maximum = int(selected['SELECTED'])
    assert key(leaf['entries'][-1]) == maximum and maximum < 2147483647
    changed = list(leaf['entries'])
    changed[-1] = encoded(2147483647, changed[-1][-REFERENCE_BYTES:])
    middle = len(changed) // 2
    assert middle == int(selected['LEFT_COUNT'])
    sizes = {'unsplit_leaf_best_bytes': best_size(changed, False),
             'left_best_bytes': best_size(changed[:middle], False),
             'right_best_bytes': best_size(changed[middle:], False)}
    assert sizes['unsplit_leaf_best_bytes'] > CAPACITY
    assert sizes['left_best_bytes'] <= CAPACITY and sizes['right_best_bytes'] <= CAPACITY
    bounds = list(parent['entries'])
    if parent['tail'] == 0:
        assert parent['children'][-1] == leaf_number
        bounds.pop()
    else:
        assert parent['tail'] == leaf_number
    bounds.append(changed[middle - 1])
    if parent['tail'] == 0:
        bounds.append(changed[-1])
    sizes['parent_best_bytes'] = best_size(bounds, True)
    assert sizes['parent_best_bytes'] > CAPACITY
    assert sizes['unsplit_leaf_best_bytes'] == int(selected['LEAF_BEST_AFTER'])
    assert sizes['parent_best_bytes'] == int(selected['PARENT_BEST_AFTER'])
    if sparse:
        assert sorted(target['rows']) == selected['keep']
        assert len(target['rows']) == len(leaves) - 1 + len(leaf['entries'])
        assert [[number, len(nodes[number]['entries']), key(nodes[number]['entries'][-1])]
                if number == leaf_number else [number, 1, key(nodes[number]['entries'][-1])]
                for number in leaves] == [
                    [number, count if number == leaf_number else 1, maximum]
                    for number, count, maximum in selected['leaves']]
    else:
        assert values == list(range(1, int(selected['INITIAL_ROWS']) + 1))
        expected_keep = []
        for number in leaves:
            expected_keep.extend([key(entry) for entry in nodes[number]['entries']]
                                 if number == leaf_number else [key(nodes[number]['entries'][-1])])
        assert sorted(expected_keep) == selected['keep']
    serial = {
        str(number): {**{field: value for field, value in node.items() if field != 'entries'},
                      'entry_count': len(node['entries']),
                      'full_key_row_sha256': hashlib.sha256(b''.join(node['entries'])).hexdigest(),
                      'maximum': node['subtree_maximum']}
        for number, node in nodes.items()
    }
    return {'root': root, 'parent': parent_number, 'leaf': leaf_number,
            'root_height': nodes[root]['height'], 'sizes': sizes,
            'parent_overflow': True, 'predicted_root_growth': parent_number == root,
            'actual_public_update_root_growth': 'not executed in producer phase',
            'nodes': serial, 'leaf_order': leaves, 'live_rows': len(values),
            'selected_value': maximum, 'selected_row': target['rows'][maximum],
            'goal_live_1536_met': len(values) <= 1536,
            'full_primary_key_row_sha256': hashlib.sha256(b''.join(entries)).hexdigest()}


def compare_history(full, sparse):
    assert full['root'] == sparse['root'] and full['parent'] == sparse['parent']
    assert full['leaf_order'] == sparse['leaf_order']
    assert full['root_height'] == sparse['root_height']
    assert set(full['nodes']) == set(sparse['nodes'])
    for number, before in full['nodes'].items():
        after = sparse['nodes'][number]
        assert before['height'] == after['height']
        assert before['children'] == after['children'] and before['tail'] == after['tail']
        assert before['previous'] == after['previous'] and before['next'] == after['next']
        assert before['maximum'] == after['maximum']
        if before['height'] > 0:
            assert before['entry_count'] == after['entry_count']
            assert before['full_key_row_sha256'] == after['full_key_row_sha256']
    return {'same_all_original_tree_pages': True, 'same_heights_child_tail_siblings': True,
            'all_subtree_maxima_retained': True,
            'actual_pruning_or_merge': False,
            'parent_bytes_before': full['sizes']['parent_best_bytes'],
            'parent_bytes_after': sparse['sizes']['parent_best_bytes']}


def future_cell(raw, metadata, number):
    start = metadata['table_page'] * PAGE_BYTES + metadata['row_start']
    end = metadata['table_page'] * PAGE_BYTES + metadata['row_end']
    row = raw[start:end]
    if row[0] == MAP_INLINE:
        base = unsigned(row, MAP_BASE_OFFSET)
        relative = number - base
        assert relative >= 0
        offset = start + MAP_BITMAP_OFFSET + relative // 8
        assert offset < end
        bit = relative % 8
    elif row[0] == MAP_INDIRECT:
        per_bitmap = (PAGE_BYTES - BITMAP_PAGE_START) * 8
        slot, relative = divmod(number, per_bitmap)
        pointer_offset = MAP_BASE_OFFSET + slot * REFERENCE_BYTES
        assert pointer_offset + REFERENCE_BYTES <= len(row)
        pointer = unsigned(row, pointer_offset)
        assert 0 < pointer < len(raw) // PAGE_BYTES
        assert raw[pointer * PAGE_BYTES:pointer * PAGE_BYTES + 2] == b'\x05\x01'
        offset = pointer * PAGE_BYTES + BITMAP_PAGE_START + relative // 8
        bit = relative % 8
    else:
        raise AssertionError('Unknown future map representation')
    return {'page': number, 'offset': offset, 'mask': 1 << bit,
            'before': raw[offset], 'set': bool(raw[offset] & (1 << bit))}


def native_order(raw, owned, table):
    result = {}
    for number in owned:
        start = number * PAGE_BYTES
        data = raw[start:start + PAGE_BYTES]
        assert data[:2] == b'\x01\x01' and unsigned(data, 4) == table['owner']
        count = unsigned(data, ROW_COUNT_OFFSET, 2)
        assert count <= MAX_SLOTS
        for slot in range(count):
            word = unsigned(data, ROW_START + slot * 2, 2)
            if word & DELETED_MASK:
                continue
            assert word & ~OFFSET_MASK == 0
            low = word & OFFSET_MASK
            high = PAGE_BYTES if slot == 0 else unsigned(data, ROW_START + (slot - 1) * 2, 2) & OFFSET_MASK
            record = data[low:high]
            assert len(record) == FIXED_RECORD_BYTES and unsigned(record, 0, COLUMN_COUNT_BYTES) == COLUMN_COUNT and record[-1] == PRESENT_MASK
            value = int.from_bytes(record[COLUMN_COUNT_BYTES:COLUMN_COUNT_BYTES + INT32_BYTES], 'little', signed=True)
            marker = int.from_bytes(record[COLUMN_COUNT_BYTES + INT32_BYTES:COLUMN_COUNT_BYTES + 2 * INT32_BYTES], 'little', signed=True)
            assert value not in result
            getter = table['rows'][value]
            assert (getter['page'], getter['slot'], getter['marker']) == (number, slot, marker)
            result[value] = len(result)
    assert set(result) == set(table['rows'])
    return result
