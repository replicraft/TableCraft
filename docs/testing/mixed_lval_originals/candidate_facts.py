"""Candidate-owned NULL-aware snapshot parser; accepted source parser untouched."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from indirect_lval_observations import capture as source

PAGE_BYTES = 4096
MAX_FILE_BYTES = 8 * 1024 * 1024
SELECTED = 'IndirectLongValues'
PADDING = 'Padding'
LONG = source.LONG
MAP = source.MAP
declarations = source.declarations
claims = source.claims
number = source.number


def snapshot_bytes(raw, text, file_identity):
    assert 0 < len(raw) <= MAX_FILE_BYTES and len(raw) % PAGE_BYTES == 0
    file_pages = len(raw) // PAGE_BYTES
    tables = {}
    maps = {}
    rows = {SELECTED: {}, PADDING: {}}
    values = {}
    raw_rows = {}
    absent = []
    entries = {}
    indexes = {}
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            name = fields[1]
            assert name not in tables
            tables[name] = dict(zip(('definition', 'rows', 'columns_count', 'max_columns'),
                                    map(int, fields[2:]), strict=True))
        elif tag == 'MAP':
            role = fields[1]
            assert role not in maps
            maps[role] = dict(zip(('table_page', 'row_start', 'row_end', 'reported_count'),
                                  map(int, fields[2:]), strict=True), all_members=[])
        elif tag == 'MEMBER':
            maps[fields[1]]['all_members'].append(int(fields[2]))
        elif tag == 'MAP_STATUS':
            absent.append(fields[1:])
        elif tag == 'ROW':
            table, identity = fields[1], int(fields[2])
            assert identity not in rows[table]
            rows[table][identity] = dict(zip(('page', 'slot', 'bytes'), map(int, fields[3:6]), strict=True),
                                         sha256=fields[6])
        elif tag == 'VALUE':
            identity = (int(fields[1]), fields[2])
            assert identity not in values
            values[identity] = None if fields[3] == 'NULL' else bytes.fromhex(fields[3])
        elif tag == 'RAWROW':
            assert int(fields[1]) not in raw_rows
            raw_rows[int(fields[1])] = bytes.fromhex(fields[2])
        elif tag == 'ENTRY':
            entries.setdefault(fields[1], []).append(int(fields[2]))
        elif tag == 'INDEX':
            indexes.setdefault(fields[1], {})[fields[2]] = dict(
                zip(('physical', 'root', 'entries'), map(int, fields[3:]), strict=True))
        else:
            assert tag in ('COLUMN', 'PROPERTY')

    expected_rows = 2
    expected_padding = 700
    assert set(rows[SELECTED]) == set(range(1, expected_rows + 1))
    assert set(rows[PADDING]) == set(range(1, expected_padding + 1))
    assert tables[SELECTED]['rows'] == expected_rows and tables[PADDING]['rows'] == expected_padding
    assert set(values) == {(identity, name) for identity in (1, 2) for name in ('Memo', 'OLE')}
    assert set(raw_rows) == {1, 2}
    assert set(entries) == {'PrimaryID', 'IDAsc', 'IDTwin'}
    assert all(order == list(range(1, expected_rows + 1)) for order in entries.values())

    inventory = {name: declarations(raw, table) for name, table in tables.items()}
    for name in (SELECTED, PADDING):
        assert inventory[name]['complete']
    assert [(column['native'], column['flags'], column['extended'])
            for column in inventory[SELECTED]['columns']] == [(4, 3, 0), (12, 2, 0), (11, 2, 0)]

    decoded = {}
    for role, metadata in maps.items():
        metadata['file_members'] = [page for page in metadata['all_members'] if page < file_pages]
        entry = MAP.decode_map(raw, metadata, PAGE_BYTES, file_pages)
        entry['raw_row_hex'] = raw[entry['row_start']:entry['row_end']].hex()
        entry['all_members'] = metadata['all_members']
        decoded[role] = entry

    global_free = set(decoded['GlobalFree']['file_members'])
    bitmap_storage = {}
    for role, entry in decoded.items():
        assert entry['map_page'] not in global_free
        for page in entry['bitmap_pages']:
            body = raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
            assert body[:4] == b'\x05\x01\x00\x00' and page not in global_free
            bitmap_storage.setdefault(page, []).append(role)
    bitmap_proofs = {page: {'roles': roles,
                           'sha256': hashlib.sha256(raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]).hexdigest(),
                           'header_hex': raw[page * PAGE_BYTES:page * PAGE_BYTES + 4].hex(),
                           'allocated': page not in global_free}
                     for page, roles in bitmap_storage.items()}

    longs = {}
    for identity, location in rows[SELECTED].items():
        final, path_frames = LONG.final_frame(raw, location)
        body = raw[final['start']:final['end']]
        assert body == raw_rows[identity]
        assert location['bytes'] == len(body)
        assert location['sha256'] == hashlib.sha256(body).hexdigest()
        assert number(body, 2, 4) == identity
        fields = {}
        for label, native, variable, column_number in (('Memo', 12, 0, 1), ('OLE', 11, 1, 2)):
            value = LONG.long_value(raw, body, {'variable': variable, 'number': column_number,
                                               'native_type': native})
            if value['kind'] == 'NULL':
                actual = None
            elif label == 'Memo':
                actual = value['logical'].encode('utf-16le')
            else:
                actual = bytes.fromhex(value['logical']['binary_hex'])
            assert values[(identity, label)] == actual
            fields[label] = value
        longs[identity] = {'path': path_frames, 'fields': fields}

    owner = tables[SELECTED]['definition']
    for column in inventory[SELECTED]['columns']:
        if column['native'] not in (11, 12):
            continue
        role = f"Table:{owner}:Column:{column['number']}"
        owned = set(decoded[role + ':Owned']['file_members'])
        candidate = set(decoded[role + ':Hint']['file_members'])
        assert candidate <= owned and owned.isdisjoint(global_free)
        for row in longs.values():
            assert all(chunk['page'] in owned for chunk in row['fields'][column['name']]['chunks'])

    role_claims = claims(raw, tables, inventory, decoded, indexes)
    return json.loads(json.dumps({'file': file_identity, 'tables': tables, 'declarations': inventory, 'maps': decoded,
            'bitmap_storage': bitmap_proofs, 'indexes': indexes, 'absent_current_holders': absent,
            'role_storage_claims': role_claims, 'rows': rows, 'long_values': longs, 'index_orders': entries}))


def snapshot(path, stdout):
    return snapshot_bytes(path.read_bytes(), stdout.read_text(), source.common.pin(path))


def compare(before, after, case):
    assert before['tables'] == after['tables']
    assert before['declarations'] == after['declarations']
    assert before['indexes'] == after['indexes']
    assert before['index_orders'] == after['index_orders']
    assert before['absent_current_holders'] == after['absent_current_holders']
    assert set(before['maps']) == set(after['maps'])
    for role, prior in before['maps'].items():
        current = after['maps'][role]
        for key in ['payload_type', 'map_page', 'map_slot', 'row_start', 'row_end', 'bitmap_pages']:
            assert prior[key] == current[key]
    for table, rows in before['rows'].items():
        for identity, row in rows.items():
            actual = after['rows'][table][identity]
            assert (actual['page'], actual['slot']) == (row['page'], row['slot'])
            if table != source.SELECTED or identity != '1':
                assert actual == row
    from mixed_lval_originals import proof

    wanted = proof.replacement(case['replacement'])
    selected = after['long_values']['1']['fields']['Memo']
    actual = None if selected['kind'] == 'NULL' else bytes.fromhex(selected['encoded_hex'])
    assert actual == wanted
    for identity in ('1', '2'):
        prior_ole = before['long_values'][identity]['fields']['OLE']
        actual_ole = after['long_values'][identity]['fields']['OLE']
        assert {key: value for key, value in prior_ole.items() if key != 'range'} == {
            key: value for key, value in actual_ole.items() if key != 'range'}
    assert before['long_values']['2'] == after['long_values']['2']
    return {'public_null': selected['kind'] == 'NULL', 'selected_encoded_sha256': selected.get('encoded_sha256'),
            'all_unselected_original_rows_preserved': True, 'all_index_metadata_orders_preserved': True,
            'all_map_role_inventory_and_geometry_preserved': True}
