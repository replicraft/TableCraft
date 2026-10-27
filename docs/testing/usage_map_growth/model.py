"""Saved raw numeric/map growth proof; importing never launches a process."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common
from indirect_lval_observations import capture as STORAGE
from integral_split_originals import model as INDEX
from parent_index_split_originals.wire import future_cell

PAGE = 4096
TARGET = 'GrowthKeys'
PADDING = 'Padding'
MAX_BYTES = 8 * 1024 * 1024


def empty_tree(raw, root, owner):
    body = raw[root * PAGE:(root + 1) * PAGE]
    assert body[:2] == b'\x04\x01'
    assert INDEX.number(body, 2, 2) == 3616 and INDEX.number(body, 4) == owner
    assert body[8:] == bytes(PAGE - 8)
    return {'pages': {root: {'height': 0, 'entries_hex': [],
                             'sha256': hashlib.sha256(body).hexdigest()}}, 'keys_hex': []}


def writer_size(entries):
    # The pinned writer derives new prefixes from its five encoded key bytes,
    # while the reader also accepts persisted prefixes containing row-ref bytes.
    assert entries and all(len(entry) == 9 for entry in entries)
    prefix = 5
    for entry in entries[1:]:
        prefix = next((at for at in range(prefix) if entry[at] != entries[0][at]), prefix)
    return 9 * len(entries) - prefix * (len(entries) - 1)


def snapshot(path, output, stage, profile):
    raw = path.read_bytes()
    assert len(raw) <= MAX_BYTES and len(raw) % PAGE == 0
    INDEX.qualify_header(raw, profile)
    tables = {}
    maps = {}
    indexes = {}
    rows = {TARGET: {}, PADDING: {}}
    values = {}
    bodies = {}
    orders = {}
    policy = []
    absent = []
    for line in output.read_text().splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            tables[fields[1]] = dict(zip(('definition', 'rows', 'columns_count', 'max_columns'),
                                        map(int, fields[2:]), strict=True))
        elif tag == 'MAP':
            assert fields[1] not in maps
            maps[fields[1]] = dict(zip(('table_page', 'row_start', 'row_end', 'reported_count'),
                                      map(int, fields[2:]), strict=True), all_members=[])
        elif tag == 'MEMBER':
            maps[fields[1]]['all_members'].append(int(fields[2]))
        elif tag == 'INDEX':
            indexes.setdefault(fields[1], {})[fields[2]] = dict(
                zip(('physical', 'root', 'entries'), map(int, fields[3:]), strict=True))
        elif tag == 'ENTRY':
            orders.setdefault(fields[1], []).append(int(fields[2]))
        elif tag == 'ROW':
            name, identity = fields[1], int(fields[2])
            assert identity not in rows[name]
            rows[name][identity] = dict(zip(('page', 'slot', 'bytes'), map(int, fields[3:6]), strict=True),
                                        sha256=fields[6])
        elif tag == 'VALUE':
            values[int(fields[1])] = {'key': int(fields[2]), 'marker': int(fields[3])}
        elif tag == 'RAWROW':
            bodies[int(fields[1])] = bytes.fromhex(fields[2])
        elif tag == 'POLICY':
            policy.append(fields[1:])
        elif tag == 'MAP_STATUS':
            absent.append(fields[1:])
        else:
            assert tag in ('COLUMN', 'PROPERTY')
    assert policy == [['false', 'true', 'false', 'UTC']]
    expected_target = 0 if stage == 'empty' else 900
    expected_padding = 700 if stage in ('before', 'after') else 0
    assert set(rows[TARGET]) == set(range(1, expected_target + 1))
    assert set(rows[PADDING]) == set(range(1, expected_padding + 1))
    assert tables[TARGET]['rows'] == expected_target and tables[PADDING]['rows'] == expected_padding
    declarations = {name: STORAGE.declarations(raw, table) for name, table in tables.items()}
    for name in (TARGET, PADDING):
        assert declarations[name]['complete']

    decoded = {}
    for role, metadata in maps.items():
        metadata['file_members'] = [page for page in metadata['all_members'] if page < len(raw) // PAGE]
        value = STORAGE.MAP.decode_map(raw, metadata, PAGE, len(raw) // PAGE)
        value['raw_row_hex'] = raw[value['row_start']:value['row_end']].hex()
        value['all_members'] = metadata['all_members']
        decoded[role] = value
    for name, records in rows.items():
        for identity, record in records.items():
            final, path_frames = STORAGE.LONG.final_frame(raw, record)
            body = raw[final['start']:final['end']]
            assert len(path_frames) == 1 and final['word'] & 0xe000 == 0
            assert len(body) == record['bytes'] and hashlib.sha256(body).hexdigest() == record['sha256']
            if name == TARGET:
                expected_key = 2147483647 if stage == 'after' and identity == 1 else 0
                assert values[identity] == {'key': expected_key, 'marker': identity * 17}
                assert body == bodies[identity] and len(body) == 15
                assert INDEX.number(body, 2) == identity
                assert INDEX.number(body, 6) == expected_key
                assert INDEX.number(body, 10) == identity * 17
            else:
                assert len(body) == 3595

    owner = tables[TARGET]['definition']
    selected = indexes[TARGET]
    assert set(selected) == {'PrimaryID', 'Ascending', 'Twin'}
    assert selected['Ascending']['physical'] == selected['Twin']['physical']
    assert selected['PrimaryID']['physical'] != selected['Ascending']['physical']
    trees = {}
    for label, metadata in selected.items():
        graph = INDEX.tree(raw, metadata['root'], owner, 4) if expected_target else empty_tree(raw, metadata['root'], owner)
        expected = []
        for identity, row in rows[TARGET].items():
            key = identity if label == 'PrimaryID' else values[identity]['key']
            reference = row['page'] << 8 | row['slot']
            expected.append(INDEX.wire(key, reference, 4, False, False))
        assert graph['keys_hex'] == [entry.hex() for entry in sorted(expected)]
        expected_order = list(range(1, expected_target + 1))
        if stage == 'after' and label != 'PrimaryID':
            expected_order = expected_order[1:] + [1]
        assert orders.get(label, []) == expected_order
        assert metadata['entries'] == expected_target
        trees[label] = graph
    claims = STORAGE.claims(raw, tables, declarations, decoded, indexes)
    return {'file': common.pin(path), 'tables': tables, 'declarations': declarations, 'maps': decoded,
            'indexes': indexes, 'trees': trees, 'rows': rows, 'values': values,
            'role_storage_claims': claims, 'absent_current_holders': absent}


def source_gate(state):
    owner = state['tables'][TARGET]['definition']
    columns = state['declarations'][TARGET]['columns']
    assert [column['name'] for column in columns] == ['ID', 'KeyValue', 'Marker']
    for column in columns:
        assert (column['native'], column['flags'], column['extended']) == (4, 3, 0)
        assert INDEX.number(bytes.fromhex(column['raw_hex']), 23, 2) == 4
    index = state['indexes'][TARGET]['Ascending']
    tree = state['trees']['Ascending']
    root = index['root']
    assert set(tree['pages']) == {root} and tree['pages'][root]['height'] == 0
    keys = [bytes.fromhex(value) for value in tree['keys_hex']]
    assert len(keys) == 900 and writer_size(keys) == 3605
    assert tree['pages'][root]['prefix'] == 5
    assert 3616 - tree['pages'][root]['free'] == writer_size(keys)
    row = state['rows'][TARGET][1]
    reference = row['page'] << 8 | row['slot']
    retained = [entry for entry in keys if int.from_bytes(entry[-4:], 'big') != reference]
    changed = sorted(retained + [INDEX.wire(2147483647, reference, 4, False, False)])
    assert writer_size(changed) == 7201
    assert writer_size(changed[:450]) == 1805 and writer_size(changed[450:]) == 3601
    owned = state['maps'][f'Table:{owner}:Index:Ascending']
    global_free = state['maps']['GlobalFree']
    assert owned['payload_type'] == 0 and owned['file_members'] == [root]
    assert owned['row_end'] - owned['row_start'] == 69
    assert global_free['payload_type'] == 1
    assert (global_free['map_page'], global_free['map_slot']) == (1, 0)
    assert state['file']['bytes'] // PAGE - root > 512
    raw = common.path(state['file']['path']).read_bytes()
    container = owned['map_page']
    assert raw[container * PAGE:container * PAGE + 2] == b'\x01\x01'
    assert INDEX.number(raw, container * PAGE + 4) == 0
    assert INDEX.number(raw, PAGE + 4) == 1
    for bitmap in global_free['bitmap_pages']:
        assert raw[bitmap * PAGE:bitmap * PAGE + 4] == b'\x05\x01\x00\x00'
        assert bitmap not in global_free['file_members']
    for page in range(len(raw) // PAGE, len(raw) // PAGE + 3):
        assert global_future(raw, global_free, page)['set']
    role = f"Table:{owner}:IndexData:{index['physical']}"
    claims = state['role_storage_claims']
    for selected in ('GlobalFree', role):
        first = claims[selected]
        for other, second in claims.items():
            if selected == other:
                continue
            assert first['reference'] != second['reference']
            assert max(first['row_start'], second['row_start']) >= min(first['row_end'], second['row_end'])
            assert set(first['bitmaps']).isdisjoint({second['container']} | set(second['bitmaps']))
            assert set(second['bitmaps']).isdisjoint({first['container']} | set(first['bitmaps']))
    return {'root': root, 'owned_reference': owned['map_page'] << 8 | owned['map_slot'],
            'before_encoded_bytes': writer_size(keys), 'after_encoded_bytes': writer_size(changed),
            'halves_encoded_bytes': [writer_size(changed[:450]), writer_size(changed[450:])]}


def global_future(raw, metadata, page):
    container = metadata['map_page']
    original = {'table_page': container,
                'row_start': metadata['row_start'] - container * PAGE,
                'row_end': metadata['row_end'] - container * PAGE}
    return future_cell(raw, original, page)


def transition(before, after):
    first = common.path(before['file']['path']).read_bytes()
    second = common.path(after['file']['path']).read_bytes()
    owner = before['tables'][TARGET]['definition']
    old = before['maps'][f'Table:{owner}:Index:Ascending']
    new = after['maps'][f'Table:{owner}:Index:Ascending']
    frontier = len(first) // PAGE
    root = before['indexes'][TARGET]['Ascending']['root']
    assert len(second) == len(first) + 3 * PAGE
    assert old['payload_type'] == 0 and new['payload_type'] == 1
    for name in ('map_page', 'map_slot', 'row_start', 'row_end'):
        assert old[name] == new[name]
    assert new['bitmap_pages'] == [frontier + 1]
    assert new['file_members'] == sorted([root, frontier, frontier + 2])
    graph = after['trees']['Ascending']
    assert set(graph['pages']) == {root, frontier, frontier + 2}
    assert graph['pages'][root]['height'] == 1
    assert sorted(len(page['entries_hex']) for number, page in graph['pages'].items() if number != root) == [450, 450]
    assert before['indexes'][TARGET] == after['indexes'][TARGET]

    # Only one fixed four-byte value may change in any existing user record.
    for name, rows in before['rows'].items():
        for identity, row in rows.items():
            current = after['rows'][name][identity]
            assert (current['page'], current['slot'], current['bytes']) == (row['page'], row['slot'], row['bytes'])
            if name != TARGET or identity != 1:
                assert current == row
    expected = bytearray(first)
    selected = before['rows'][TARGET][1]
    physical, _ = STORAGE.LONG.final_frame(first, selected)
    expected[physical['start'] + 6:physical['start'] + 10] = (2147483647).to_bytes(4, 'little')
    expected[root * PAGE:(root + 1) * PAGE] = second[root * PAGE:(root + 1) * PAGE]
    row = bytearray(old['row_end'] - old['row_start'])
    assert len(row) == 69
    row[0] = 1
    row[1:5] = (frontier + 1).to_bytes(4, 'little')
    expected[old['row_start']:old['row_end']] = row
    global_map = before['maps']['GlobalFree']
    for number in range(frontier, frontier + 3):
        cell = global_future(first, global_map, number)
        offset, mask = cell['offset'], cell['mask']
        assert first[offset] & mask
        expected[offset] &= ~mask
    assert expected == second[:len(first)]
    bitmap = bytearray(PAGE)
    bitmap[:4] = b'\x05\x01\x00\x00'
    for number in (root, frontier, frontier + 2):
        bitmap[4 + number // 8] |= 1 << (number % 8)
    assert second[(frontier + 1) * PAGE:(frontier + 2) * PAGE] == bitmap
    return {'existing_prefix_exact_outside_fixed_value_root_map_and_global_cells': True,
            'source_eof': frontier, 'appended_index_pages': [frontier, frontier + 2],
            'appended_bitmap_page': frontier + 1, 'owned_added': [frontier, frontier + 2],
            'global_allocated': list(range(frontier, frontier + 3)), 'old_root_retained': root}
