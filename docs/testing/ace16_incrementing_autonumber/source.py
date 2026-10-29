"""Raw original population and counter facts; never an Increment-mode detector."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_global_bootstrap import source as native
from ace16_insert_empty_owned import source as ordinary
from ace16_set_null_deletes import source as records

PAGE = 4096


def parse(text):
    lines = []
    counters = {}
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'AUTONUMBER':
            assert len(fields) == 5 and fields[4] == 'false'
            name = native.label(fields[1])
            assert name not in counters
            counters[name] = [int(fields[2]), int(fields[3])]
        else:
            lines.append(line)
    return (*native.parse('\n'.join(lines)), counters)


def population(raw, table, maps):
    owner = 'Table:' + str(table['definition'])
    pages = maps[owner + ':Owned']['file_members']
    assert set(maps[owner + ':Hint']['file_members']) <= set(pages)
    live = {}
    deleted = []
    physical = 0
    for number in pages:
        start = number * PAGE
        body = raw[start:start + PAGE]
        assert body[:2] == b'\x01\x01'
        assert native.number(body, 4, 4) == table['definition']
        count = native.number(body, 12)
        assert count <= 255
        physical += count
        end = PAGE
        for slot in range(count):
            word = native.number(body, 14 + 2 * slot)
            begin = word & 0x1fff
            flags = word & 0xe000
            assert flags in (0, 0x8000, 0xc000)
            assert 14 + count * 2 <= begin < end <= PAGE
            root = number << 8 | slot
            frame = body[begin:end]
            if flags:
                deleted.append(dict(reference=root, flags=flags, raw=frame.hex()))
            else:
                assert root not in live
                assert len(frame) == 15 and frame[:2] == b'\x03\x00' and frame[-1] == 7
                live[root] = [int.from_bytes(frame[2 + 4 * column:6 + 4 * column], 'little', signed=True)
                              for column in range(3)]
            end = begin
        assert native.number(body, 2) == end - 14 - count * 2
    assert len(live) == table['count']
    return live, deleted, physical


def key(value):
    assert -(1 << 31) <= value < 1 << 31
    return b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')


def inspect(path, getter, case):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8388608 and len(raw) % PAGE == 0
    assert raw[4:19] == b'Standard ACE DB' and raw[20] == case['header']
    tables, descriptions, properties, absent, counters = parse(Path(getter).read_text())
    assert {name for name in tables if not name.startswith('MSys') and not name.startswith('f_')} == {'Items', 'Sentinel'}
    assert set(counters) == set(tables)
    maps = {}
    for role, item in descriptions.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [number for number in item['all_members'] if number < len(raw) // PAGE]
        maps[role] = native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    declarations = {name: native.declarations.declarations(raw, table) for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = native.declarations.claims(raw, tables, declarations, maps,
                                       {name: table['indexes'] for name, table in tables.items()})
    item = tables['Items']
    definition, definition_pages = native.declarations.definition(raw, item['definition'])
    owner = 'Table:' + str(item['definition'])
    assert list(item['columns']) == ['Earlier', 'ID', 'Score']
    assert item['columns_count'] == item['max_columns'] == native.number(definition, 41) == native.number(definition, 45) == 3
    assert native.number(definition, 43) == 0
    assert item['count'] == len(case['live_ids']) == len(item['rows'])
    assert native.number(definition, 16, 4) == item['count']
    assert not properties.get(owner) and not declarations['Items']['pairs']
    for position, declaration in enumerate(declarations['Items']['columns']):
        block = bytes.fromhex(declaration['raw_hex'])
        column = item['columns'][['Earlier', 'ID', 'Score'][position]]
        assert declaration['number'] == column['number'] == column['position'] == position
        assert declaration['native'] == column['native_type'] == 4
        assert declaration['flags'] == (7 if position == 1 else 3)
        assert declaration['extended'] == 0
        assert native.number(block, 21) == position * 4 and native.number(block, 23) == column['length'] == 4
        assert column['auto'] == (position == 1) and not column['compressed']
        # This exact factory profile has no stored mode/default properties.
        # Absence is recorded, never treated as a Random/Increment discriminator.
        assert not properties.get(owner + ':Column:' + str(position))
    last = int.from_bytes(definition[20:24], 'little', signed=True)
    complex_last = int.from_bytes(definition[28:32], 'little', signed=True)
    assert last == case['expected_last'] and complex_last == 0
    assert counters['Items'] == [last, complex_last]
    live, deleted, physical = population(raw, item, maps)
    assert physical == case['generated_count']
    assert len(deleted) == len(case['deleted_ids'])
    assert sorted(values[1] for values in live.values()) == case['live_ids']
    for reference, values in live.items():
        earlier, identity, score = values
        assert [earlier, score] == [700 + identity, 10 if identity % 2 else 20]
        assert item['values'][identity] == dict(Earlier=earlier, ID=identity, Score=score)
        reported = item['rows'][identity]
        assert (reported['page'] << 8 | reported['slot']) == reference
        assert reported['target_page'] == reported['page'] and reported['target_slot'] == reported['slot']
        page = (reference >> 8) * PAGE
        slot = reference & 255
        start = native.number(raw, page + 14 + slot * 2) & 0x1fff
        end = PAGE if slot == 0 else native.number(raw, page + 12 + slot * 2) & 0x1fff
        assert reported['raw'] == raw[page + start:page + end]
    removed_ids = []
    for row in deleted:
        frame = bytes.fromhex(row['raw'])
        assert len(frame) == 15 and frame[:2] == b'\x03\x00' and frame[-1] == 7
        identity = int.from_bytes(frame[6:10], 'little', signed=True)
        assert int.from_bytes(frame[2:6], 'little', signed=True) == 700 + identity
        assert int.from_bytes(frame[10:14], 'little', signed=True) == (10 if identity % 2 else 20)
        removed_ids.append(identity)
    assert sorted(removed_ids) == case['deleted_ids']
    logical, index_definition_pages = records.index_declarations(raw, item, maps)
    assert index_definition_pages == definition_pages
    assert set(logical) == {'PrimaryID', 'PrimaryAlias', 'ScoreOrder', 'ScoreAlias'}
    assert set(index['physical'] for index in item['indexes'].values()) == {0, 1}
    physical_pages = {}
    for name, index in item['indexes'].items():
        primary = name in ('PrimaryID', 'PrimaryAlias')
        assert index['physical'] == (0 if primary else 1)
        assert index['columns'] == [('ID' if primary else 'Score', True)]
        assert index['flags'] == (0x89 if primary else 0x80)
        assert logical[name]['kind'] == (1 if name == 'PrimaryID' else 0)
        assert logical[name]['role'] == logical[name]['update'] == logical[name]['delete'] == 0
        assert index['history'] == (case['generated_count'] if primary else min(case['generated_count'], 2))
        nodes, leaves, stream = native.index_wire.tree(raw, index['root'], item['definition'])
        assert len(nodes) == len(leaves) == 1
        field = 1 if primary else 2
        eligible = sorted((key(values[field]), reference) for reference, values in live.items())
        assert stream == [wire + reference.to_bytes(4, 'big') for wire, reference in eligible]
        assert index['entries'] == len(live)
        assert index['rows'] == [(live[reference][1], reference >> 8, reference & 255)
                                 for wire, reference in eligible]
        assert maps[owner + ':IndexData:' + str(index['physical'])]['file_members'] == list(nodes)
        if index['physical'] in physical_pages:
            assert physical_pages[index['physical']] == set(nodes)
        else:
            physical_pages[index['physical']] = set(nodes)
        if case['state'] == 'pressure':
            page = raw[index['root'] * PAGE:(index['root'] + 1) * PAGE]
            assert native.number(page, 24) == (3 if primary else 4)
            assert native.number(page, 2) == (19 if primary else 617)
    sentinel = tables['Sentinel']
    assert list(sentinel['columns']) == ['ID', 'Marker'] and not sentinel['indexes']
    assert sentinel['count'] == 1 and sentinel['values'] == {91:dict(ID=91, Marker=-12345)}
    assert not properties.get('Table:' + str(sentinel['definition']))
    assert not declarations['Sentinel']['pairs']
    for column in declarations['Sentinel']['columns']:
        assert column['native'] == 4 and column['flags'] == 3 and column['extended'] == 0
        assert not properties.get('Table:' + str(sentinel['definition']) + ':Column:' + str(column['number']))
    sentinel_records = records.live_records(raw, sentinel, maps)
    assert len(sentinel_records) == 1
    assert counters['Sentinel'] == [0, 0]
    for reference, body in sentinel_records:
        values = {column['name']: records.scalar_value(body, column)
                  for column in declarations['Sentinel']['columns']}
        assert values == sentinel['values'][91]
        assert body == sentinel['rows'][91]['raw']
        assert reference == (sentinel['rows'][91]['page'] << 8 | sentinel['rows'][91]['slot'])
    for name, table in tables.items():
        body, _ = native.declarations.definition(raw, table['definition'])
        assert counters[name] == [int.from_bytes(body[20:24], 'little', signed=True),
                                  int.from_bytes(body[28:32], 'little', signed=True)]
    selected = [role for role in claims if any(role.startswith('Table:' + str(tables[name]['definition']) + ':')
                                              for name in ('Items', 'Sentinel'))]
    ordinary.require_exclusive(claims, selected)
    role_sets = [set(maps['Table:' + str(tables[name]['definition']) + ':Owned']['file_members'])
                 for name in ('Items', 'Sentinel')] + list(physical_pages.values())
    metadata = set()
    for claim in claims.values():
        metadata.add(claim['container'])
        metadata.update(claim['bitmaps'])
    for table in tables.values():
        _, pages = native.declarations.definition(raw, table['definition'])
        metadata.update(pages)
    for position, members in enumerate(role_sets):
        assert members.isdisjoint(metadata)
        assert all(members.isdisjoint(other) for other in role_sets[:position])
    free = set(maps['GlobalFree']['file_members'])
    for name in ('Items', 'Sentinel'):
        prefix = 'Table:' + str(tables[name]['definition'])
        for role, item_map in maps.items():
            if role == prefix + ':Owned' or role.startswith(prefix + ':IndexData:'):
                assert not set(item_map['file_members']) & free
    eof = len(raw) // PAGE
    global_map = maps['GlobalFree']
    assert global_map['payload_type'] == 0
    global_start = global_map['row_start']
    base = native.number(raw, global_start + 1, 4)
    for number in range(eof, eof + 8):
        relative = number - base
        assert relative >= 0
        offset = global_start + 5 + relative // 8
        assert offset < global_map['row_end'] and raw[offset] & (1 << (relative % 8))
    catalog = records.catalog_long_values(raw, tables, declarations, maps, claims)
    return dict(raw=raw, tables=tables, maps=maps, claims=claims, declarations=declarations,
                logical=logical, live_rows=live, deleted_rows=deleted, physical_rows=physical,
                counter_last=last, complex_last=complex_last,
                stored_mode='unclassified; explicit caller engineering policy required',
                raw_table_24_28=definition[24:28].hex(), catalog_long_values=catalog,
                protected_prefix_sha256=hashlib.sha256(raw).hexdigest())
