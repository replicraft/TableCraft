"""Saved original bytes, reciprocal foreign indexes and exact relationship actions."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_global_bootstrap import source as native
from ace16_insert_empty_owned import source as ordinary

PAGE = 4096
PRIMARY = 1
FOREIGN = 2
ROW_BYTES = 4


def parse(text):
    ordinary_lines = []
    entries = {}
    relationships = {}
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'ENTRY':
            assert len(fields) == 6
            table, index = map(native.label, fields[1:3])
            entries.setdefault((table, index), []).append(tuple(map(int, fields[3:])))
        elif fields[0] == 'RELATIONSHIP':
            assert len(fields) == 8
            name, parent, child = map(native.label, fields[1:4])
            assert name not in relationships
            assert all(item in ('true', 'false') for item in fields[4:])
            relationships[name] = (parent, child, *[item == 'true' for item in fields[4:]])
        else:
            ordinary_lines.append(line)
    tables, maps, properties, absent = native.parse('\n'.join(ordinary_lines))
    for (table, index), rows in entries.items():
        tables[table]['indexes'][index]['rows'] = rows
    return tables, maps, properties, absent, relationships


def scalar_value(body, declaration):
    block = bytes.fromhex(declaration['raw_hex'])
    count = native.number(body, 0)
    mask = len(body) - (count + 7) // 8
    number = declaration['number']
    assert number < count and 2 <= mask < len(body)
    if not body[mask + number // 8] & (1 << (number % 8)):
        return None
    if declaration['native'] in (3, 4):
        width = 2 if declaration['native'] == 3 else 4
        start = 2 + native.number(block, 21)
        assert start + width <= mask
        return int.from_bytes(body[start:start + width], 'little', signed=True)
    assert declaration['native'] == 10 and declaration['flags'] & 1 == 0
    variables = native.number(body, mask - 2)
    ordinal = native.number(block, 7)
    assert ordinal < variables
    footer = mask - 2 - 2 * (variables + 1)
    start = native.number(body, mask - 4 - ordinal * 2)
    end = native.number(body, mask - 6 - ordinal * 2)
    assert 2 <= start <= end <= footer
    payload = body[start:end]
    return native.long_wire.decode_stream(payload)


def live_records(raw, table, maps):
    pages = maps['Table:' + str(table['definition']) + ':Owned']['file_members']
    records = []
    roots = set()
    for number in pages:
        base = number * PAGE
        page = raw[base:base + PAGE]
        assert page[:2] == b'\x01\x01' and native.number(page, 4, 4) == table['definition']
        count = native.number(page, 12)
        assert count <= 255
        end = PAGE
        for slot in range(count):
            word = native.number(page, 14 + slot * 2)
            start = word & 0x1fff
            assert not word & 0x2000 and 14 + count * 2 <= start <= end
            if not word & 0x8000:
                root = number << 8 | slot
                frame, chain = native.long_wire.final_frame(raw, dict(page=number, slot=slot))
                assert all(part['page'] in pages for part in chain)
                assert all(native.number(raw, part['page'] * PAGE + 4, 4) == table['definition']
                           for part in chain)
                body = raw[frame['start']:frame['end']]
                assert start < end and root not in roots
                roots.add(root)
                records.append((root, body))
            end = start
        assert native.number(page, 2) == end - 14 - count * 2
    assert len(records) == table['count']
    return records


def map_reference(metadata):
    page = metadata['map_page']
    slot = metadata['map_slot']
    assert 0 < page < (1 << 24) and 0 <= slot < 256
    return page << 8 | slot


def index_declarations(raw, table, maps):
    data, pages = native.declarations.definition(raw, table['definition'])
    physical_count = native.number(data, 51, 4)
    logical_count = native.number(data, 47, 4)
    assert logical_count == len(table['indexes']) <= 32
    assert native.number(data, 16, 4) == table['count']
    owner = 'Table:' + str(table['definition'])
    for offset, suffix in ((55, ':Owned'), (59, ':Hint')):
        assert native.number(data, offset, 4) == map_reference(maps[owner + suffix])
    at = 63 + physical_count * 12 + table['columns_count'] * 25
    for column in table['columns']:
        length = native.number(data, at)
        at += 2
        assert data[at:at + length].decode('utf-16le') == column
        at += length
    physical = [data[at + number * 52:at + (number + 1) * 52]
                for number in range(physical_count)]
    logical_start = at + physical_count * 52
    names = logical_start + logical_count * 28
    logical = {}
    for number in range(logical_count):
        block = data[logical_start + number * 28:logical_start + (number + 1) * 28]
        length = native.number(data, names)
        names += 2
        name = data[names:names + length].decode('utf-16le')
        names += length
        assert name not in logical and native.number(block, 4, 4) == number
        index = table['indexes'][name]
        position = native.number(block, 8, 4)
        assert position == index['physical'] < physical_count
        header = physical[position]
        assert native.number(header, 38, 4) == index['root']
        assert native.number(header, 34, 4) == map_reference(maps[owner + ':IndexData:' + str(position)])
        assert header[46] == index['flags'] and index['flags'] & ~0x8b == 0
        assert index['history_offset'] == 63 + position * 12 + 4
        assert native.number(data, index['history_offset'], 4) == index['history']
        assert len(index['columns']) == 1 and index['columns'][0][1] is True
        key_column = table['columns'][index['columns'][0][0]]['number']
        assert native.number(header, 4) == key_column and header[6] == 1
        assert all(native.number(header, 4 + item * 3) == 0xffff for item in range(1, 10))
        assert block[23] in (0, PRIMARY, FOREIGN)
        assert index['primary'] == (block[23] == PRIMARY)
        logical[name] = dict(number=number, physical=position, kind=block[23],
                             role=block[12], target_index=native.number(block, 13, 4),
                             target_table=native.number(block, 17, 4),
                             update=block[21], delete=block[22])
    assert native.number(data, names) == 0xffff
    return logical, pages


def numeric_key(value):
    if value is None:
        return b'\x00'
    return b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')


def catalog_long_values(raw, tables, declarations, maps, claims):
    ownership = {}
    for name, declaration in declarations.items():
        definition = tables[name]['definition']
        for pair in declaration['pairs']:
            assert pair['owned'] and pair['candidate']
            role = 'Table:' + str(definition) + ':Column:' + str(pair['column']) + ':Owned'
            for page in maps[role]['file_members']:
                ownership.setdefault(page, set()).add((definition, pair['column']))
    result = {}
    for name, table in tables.items():
        columns = [item for item in declarations[name]['columns'] if item['native'] in (11, 12)]
        if not columns:
            continue
        live = live_records(raw, table, maps)
        traces = 0
        for root, body in live:
            count = native.number(body, 0)
            mask = len(body) - (count + 7) // 8
            for item in columns:
                number = item['number']
                if number >= count or not body[mask + number // 8] & (1 << (number % 8)):
                    continue
                block = bytes.fromhex(item['raw_hex'])
                column = dict(number=number, native_type=item['native'],
                              variable=native.number(block, 7))
                value = native.long_wire.long_value(raw, body, column)
                for chunk in value['chunks']:
                    owners = ownership.get(chunk['page'], set())
                    assert not owners or owners == {(table['definition'], number)}
                    traces += 1
        result[name] = dict(live_parent_rows=len(live), chunk_references=traces)
    return result


def inspect(path, getter, case):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8388608 and len(raw) % PAGE == 0
    assert raw[4:19] == b'Standard ACE DB' and raw[20] == 5
    tables, descriptions, properties, absent, relationships = parse(Path(getter).read_text())
    maps = {}
    for role, item in descriptions.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [page for page in item['all_members'] if page < len(raw) // PAGE]
        maps[role] = native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    declarations = {name: native.declarations.declarations(raw, table) for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = native.declarations.claims(raw, tables, declarations, maps,
                                       {name: table['indexes'] for name, table in tables.items()})
    user_names = set(case['before'])
    assert {name for name in tables if not name.startswith('MSys') and not name.startswith('f_')} == user_names
    logical = {}
    roots = {}
    for name, expected in case['before'].items():
        table = tables[name]
        columns = ['ID', 'ParentID', 'Marker'] if 'Children' in name else ['ID', 'Marker']
        assert list(table['columns']) == columns
        assert table['columns_count'] == table['max_columns'] == len(columns)
        assert table['count'] == len(expected) == len(table['rows'])
        assert table['values'] == {row[0]:dict(zip(columns, row, strict=True)) for row in expected}
        owner = 'Table:' + str(table['definition'])
        assert not properties.get(owner) and not declarations[name]['pairs']
        for position, declared in enumerate(declarations[name]['columns']):
            block = bytes.fromhex(declared['raw_hex'])
            column = table['columns'][columns[position]]
            assert declared['number'] == column['number'] == column['position'] == position
            assert declared['native'] == column['native_type'] == 4
            assert declared['flags'] == 3 and declared['extended'] == 0
            assert native.number(block, 23) == 4 and native.number(block, 21) == position * 4
            assert not column['auto'] and not column['compressed']
            wanted = [['Required', 1, 'true', 'java.lang.Boolean', 'true']] if position == 0 else []
            if columns[position] == 'ParentID':
                wanted = [['Required', 1, 'true', 'java.lang.Boolean', 'false']]
            assert properties.get(owner + ':Column:' + str(position), []) == wanted
        records = live_records(raw, table, maps)
        roots[name] = sorted(root for root, body in records)
        by_root = {row['page'] << 8 | row['slot']: identity for identity, row in table['rows'].items()}
        assert set(by_root) == set(roots[name])
        for root, body in records:
            assert native.number(body, 0) == len(columns)
            values = {item['name']:scalar_value(body,item) for item in declarations[name]['columns']}
            assert values == table['values'][by_root[root]]
            assert body == table['rows'][by_root[root]]['raw']
        logical[name], _ = index_declarations(raw, table, maps)
        primary = logical[name]['PrimaryID']
        index = table['indexes']['PrimaryID']
        assert primary['kind'] == PRIMARY and index['flags'] & 9 == 9
        assert index['columns'] == [('ID', True)]
        for index_name, index in table['indexes'].items():
            nodes, leaves, stream = native.index_wire.tree(raw, index['root'], table['definition'])
            assert len(nodes) == len(leaves) == 1
            field = index['columns'][0][0]
            eligible = [(numeric_key(table['values'][identity][field]), root)
                        for root, identity in by_root.items()
                        if table['values'][identity][field] is not None or not index['flags'] & 2]
            eligible.sort()
            assert stream == [key + root.to_bytes(ROW_BYTES,'big') for key,root in eligible]
            assert index['entries'] == len(eligible)
            assert index['history'] == len({key for key,root in eligible})
            assert index['rows'] == [(by_root[root],root >> 8,root & 255) for key,root in eligible]
            assert maps[owner + ':IndexData:' + str(index['physical'])]['file_members'] == list(nodes)
            if index['flags'] & 1:
                nonnull = [key for key,root in eligible if key != b'\x00']
                assert len(nonnull) == len(set(nonnull))
    expected_relations = {'ParentChildNull':('Parents','Children',True,False,False,True)}
    if case['selected'] == 'mixed':
        expected_relations['ParentCascade'] = ('Parents','CascadeChildren',True,False,True,False)
    assert relationships == expected_relations
    relation_rows = []
    relation_table = tables['MSysRelationships']
    for root, body in live_records(raw, relation_table, maps):
        values = {item['name']:scalar_value(body,item)
                  for item in declarations['MSysRelationships']['columns']}
        if values['szRelationship'] in expected_relations:
            relation_rows.append(values)
    assert len(relation_rows) == len(expected_relations)
    for name, (parent, child, enforced, updates, cascade, set_null) in expected_relations.items():
        row = next(row for row in relation_rows if row['szRelationship'] == name)
        assert row['grbit'] == (0x2000 if set_null else 0x1000)
        assert row['ccolumn'] == 1 and row['icolumn'] == 0
        assert (row['szReferencedObject'],row['szObject']) == (parent,child)
        assert (row['szReferencedColumn'],row['szColumn']) == ('ID','ParentID')
        endpoints = []
        for table_name, target_name, role in ((parent,child,1),(child,parent,2)):
            target_definition = tables[target_name]['definition']
            matches = [(index_name,item) for index_name,item in logical[table_name].items()
                       if item['kind'] == FOREIGN and item['role'] == role
                       and item['target_table'] == target_definition]
            assert len(matches) == 1
            index_name, item = matches[0]
            assert item['update'] == 0 and item['delete'] == (2 if set_null else 1)
            assert tables[table_name]['indexes'][index_name]['columns'] == [('ID' if role == 1 else 'ParentID', True)]
            endpoints.append(item)
        assert endpoints[0]['target_index'] == endpoints[1]['number']
        assert endpoints[1]['target_index'] == endpoints[0]['number']
        parent_ids = set(tables[parent]['rows'])
        assert all(values['ParentID'] is None or values['ParentID'] in parent_ids
                   for values in tables[child]['values'].values())
    selected_claims = [role for role in claims if any(role.startswith('Table:' + str(tables[name]['definition']) + ':')
                                                    for name in user_names)]
    ordinary.require_exclusive(claims, selected_claims)
    global_map = maps['GlobalFree']
    for name in user_names:
        owner = 'Table:' + str(tables[name]['definition'])
        for role, item in maps.items():
            if role == owner + ':Owned' or role.startswith(owner + ':IndexData:'):
                assert not set(item['file_members']) & set(global_map['file_members'])
    catalog = catalog_long_values(raw, tables, declarations, maps, claims)
    return dict(raw=raw,tables=tables,maps=maps,claims=claims,declarations=declarations,
                catalog_long_values=catalog,
                logical=logical,roots=roots,relationships=relation_rows,
                protected_prefix_sha256=hashlib.sha256(raw).hexdigest())
