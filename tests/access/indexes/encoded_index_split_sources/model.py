"""Read-only raw framing/capacity proof against independent typed/key getters."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
import capture_delete_trees as TREE
import delete_reference_maps as MAP
from integral_split_originals.model import qualify_header

PAGE = 4096
ENTRY_START = 480
CAPACITY = PAGE - ENTRY_START
TABLES = ('TextLegacy', 'TextModern', 'BinaryOne', 'LongSentinel')
TARGETS = TABLES[:3]
ROWS = 900
PUBLIC_ORDINARY_INDEX_FLAGS = 0x80


def number(raw, offset, width=4):
    return int.from_bytes(raw[offset:offset + width], 'little')


def boolean(token):
    assert token in ('true', 'false'), 'Unknown boolean protocol token'
    return token == 'true'


def packed(keys):
    """Pinned public key-only prefix framing, not engineering full-ROW packing."""
    assert keys
    prefix = min(map(len, keys))
    for key in keys[1:]:
        shared = 0
        while shared < prefix and keys[0][shared] == key[shared]:
            shared += 1
        prefix = shared
    return sum(len(key) + 4 for key in keys) - prefix * (len(keys) - 1)


def snapshot(path, getter, profile):
    changed = 'none'
    raw = path.read_bytes()
    assert len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    qualify_header(raw, profile)
    tables = {}
    maps = {}
    keys = {}
    encoders = {}
    properties = []
    property_names = set()
    key_references = {}
    map_members = {}
    policy = []
    for line in getter.read_text().splitlines():
        cells = line.split('\t')
        tag = cells[0]
        if tag == 'TABLE':
            assert cells[1] not in tables
            tables[cells[1]] = {
                'definition': int(cells[2]),
                'count': int(cells[3]),
                'columns': {},
                'rows': {},
                'indexes': {},
            }
        elif tag == 'COLUMN':
            assert cells[2] not in tables[cells[1]]['columns']
            tables[cells[1]]['columns'][cells[2]] = {
                'number': int(cells[3]),
                'native': int(cells[4]),
                'length': int(cells[5]),
                'compressed': boolean(cells[6]),
                'sort': cells[7],
                'position': int(cells[8]),
                'variable': int(cells[9]),
            }
        elif tag == 'PROPERTY':
            property_key = (cells[1], cells[2])
            assert property_key not in property_names
            property_names.add(property_key)
            properties.append(cells[1:])
        elif tag == 'ROW':
            assert int(cells[2]) not in tables[cells[1]]['rows']
            tables[cells[1]]['rows'][int(cells[2])] = {
                'page': int(cells[3]),
                'slot': int(cells[4]),
                'body': cells[5],
                'values': {},
            }
        elif tag == 'VALUE':
            assert cells[3] not in tables[cells[1]]['rows'][int(cells[2])]['values']
            tables[cells[1]]['rows'][int(cells[2])]['values'][cells[3]] = cells[4]
        elif tag == 'INDEX':
            assert cells[2] not in tables[cells[1]]['indexes']
            tables[cells[1]]['indexes'][cells[2]] = {
                'physical': int(cells[3]),
                'root': int(cells[4]),
                'count': int(cells[5]),
                'flags': int(cells[6]),
                'primary': boolean(cells[7]),
                'unique': boolean(cells[8]),
                'foreign': boolean(cells[9]),
                'columns': [],
            }
        elif tag == 'INDEX_COLUMN':
            columns = tables[cells[1]]['indexes'][cells[2]]['columns']
            assert all(column['number'] != int(cells[3]) for column in columns)
            columns.append({
                'number': int(cells[3]),
                'ascending': boolean(cells[4]),
            })
        elif tag == 'KEY':
            references = key_references.setdefault((cells[1], cells[2]), set())
            reference = (int(cells[3]), int(cells[4]))
            assert reference not in references
            references.add(reference)
            keys.setdefault((cells[1], cells[2]), []).append((int(cells[3]), int(cells[4]), cells[5]))
        elif tag == 'ENCODE':
            assert (cells[1], cells[2]) not in encoders
            encoders[(cells[1], cells[2])] = (cells[3], cells[4])
        elif tag == 'MAP':
            assert cells[1] not in maps
            maps[cells[1]] = {
                'table_page': int(cells[2]),
                'row_start': int(cells[3]),
                'row_end': int(cells[4]),
                'reported_count': int(cells[5]),
                'members': [],
            }
        elif tag == 'MEMBER':
            members = map_members.setdefault(cells[1], set())
            assert int(cells[2]) not in members
            members.add(int(cells[2]))
            maps[cells[1]]['members'].append(int(cells[2]))
        elif tag == 'POLICY':
            policy.append(cells[1:])
        elif tag == 'MAP_STATUS':
            raise AssertionError('Expected concrete sentinel LVAL maps, not absent holder')
        else:
            raise AssertionError(tag)
    assert policy == [['false', 'true', 'false', 'UTC']]
    assert set(tables) == set(TABLES)
    expected_maps = {'GlobalFree'}
    for name in TABLES:
        expected_maps.update((f'{name}:Owned', f'{name}:Hint'))
        labels = ('PrimaryID',) if name == 'LongSentinel' else ('PrimaryID', 'Ascending', 'Twin', 'Descending')
        expected_maps.update(f'{name}:Index:{label}' for label in labels)
    for column in ('Memo', 'Ole'):
        expected_maps.update((f'LongSentinel:{column}:Owned', f'LongSentinel:{column}:Hint'))
    assert set(maps) == expected_maps
    assert set(encoders) == {(name, label) for name in TARGETS for label in ('Ascending', 'Twin', 'Descending')}
    expected_key_roles = {
        (name, label)
        for name, table in tables.items()
        for label in table['indexes']
    }
    assert set(keys) == expected_key_roles
    property_owners = set(TABLES)
    for name, table in tables.items():
        property_owners.update(f'{name}:{label}' for label in table['columns'])
    assert all(property[0] in property_owners for property in properties)
    for metadata in maps.values():
        metadata['file_members'] = [page for page in metadata['members'] if page < len(raw) // PAGE]
        decoded = MAP.decode_map(raw, metadata, PAGE, len(raw) // PAGE)
        metadata['decoded'] = {key: value for key, value in decoded.items() if key != 'bit_locations'}
    for name, table in tables.items():
        count = 1 if name == 'LongSentinel' else ROWS
        assert table['count'] == len(table['rows']) == count
        expected_columns = {'ID', 'Memo', 'Ole'} if name == 'LongSentinel' else {'ID', 'KeyValue', 'Marker'}
        assert set(table['columns']) == expected_columns
        assert {column['number'] for column in table['columns'].values()} == {0, 1, 2}
        assert {column['position'] for column in table['columns'].values()} == {0, 1, 2}
        definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
        physical_count = number(definition, 51)
        fields = 63 + physical_count * 12
        references = {}
        for identity, row in table['rows'].items():
            assert 1 <= identity <= count
            assert row['page'] in maps[f'{name}:Owned']['file_members']
            assert number(raw, row['page'] * PAGE + 4) == table['definition']
            start, end = frame(raw, row['page'], row['slot'])
            body = bytes.fromhex(row['body'])
            assert set(row['values']) == expected_columns
            assert raw[start:end] == body
            assert number(body, 0, 2) == len(table['columns']) == 3
            assert body[-1] & 7 == 7
            reference = (row['page'] << 8) | row['slot']
            assert reference not in references
            references[reference] = identity
            assert row['values']['ID'] == f'Int32:{identity}'
            fixed_labels = ('ID',) if name == 'LongSentinel' else ('ID', 'Marker')
            row['fixed'] = {}
            for label in fixed_labels:
                column = table['columns'][label]
                position = fields + column['position'] * 25
                block = definition[position:position + 25]
                assert len(block) == 25 and block[0] == 4 and block[15] & 1
                fixed_offset = number(block, 21, 2)
                begin = 2 + fixed_offset
                end = begin + 4
                assert 2 <= begin < end <= len(body)
                decoded = int.from_bytes(body[begin:end], 'little', signed=True)
                expected = identity if label == 'ID' else identity * 17
                assert decoded == expected
                row['fixed'][label] = {'begin': begin, 'end': end, 'value': decoded}
            if name == 'LongSentinel':
                assert row['values']['Memo'] == 'Text:' + ('Ā' * 33).encode('utf-16le').hex()
                assert row['values']['Ole'] == 'Binary:00ff41'
                continue
            assert row['values']['Marker'] == f'Int32:{identity * 17}'
            selected = name == changed and identity == 1
            if name == 'BinaryOne':
                expected = 'ff' if selected else '00'
            else:
                expected = ('zzz' if selected else 'aaa').encode('utf-16le').hex()
            carrier = 'Binary:' if name == 'BinaryOne' else 'Text:'
            assert row['values']['KeyValue'] == carrier + expected
            assert number(body, 0, 2) == 3
            mask = len(body) - 1
            assert number(body, mask - 2, 2) == 1 and body[mask] & 2
            begin = number(body, mask - 4, 2)
            stop = number(body, mask - 6, 2)
            assert 2 <= begin <= stop <= mask - 6
            assert all(field['end'] <= begin for field in row['fixed'].values())
            assert body[begin:stop].hex() == expected
            row['selected'] = {
                'offset': start + begin,
                'length': stop - begin,
                'wire': expected,
            }
            assert stop - begin == (1 if name == 'BinaryOne' else 6)
        for label, column in table['columns'].items():
            position = column['position']
            block = definition[fields + position * 25:fields + (position + 1) * 25]
            assert len(block) == 25 and block[0] == column['native'] and number(block, 5, 2) == column['number']
            column['flags'] = block[15]
            column['extended'] = block[16]
            if label == 'KeyValue':
                assert column['variable'] == 0
                assert (column['flags'], column['extended']) == (2, 0)
                assert column['native'] == (9 if name == 'BinaryOne' else 10)
                assert column['length'] == (255 if name == 'BinaryOne' else 510)
                if name != 'BinaryOne':
                    assert column['sort'] == ('1033:0:0' if name == 'TextLegacy' else '1033:0:1')
                    assert not column['compressed']
        geometry = {
            'page_bytes': PAGE,
            'mask_offset': 27,
            'mask_bytes': 453,
            'definition_page': table['definition'],
        }
        pages = TREE.pages(raw, {'geometry': geometry})
        for label, index in table['indexes'].items():
            native = keys[(name, label)]
            observed = []
            for page, slot, key in native:
                reference = ((page << 8) | slot).to_bytes(4, 'big')
                observed.append(bytes.fromhex(key) + reference)
            assert len(observed) == index['count'] == count
            assert not index['foreign']
            if label == 'PrimaryID':
                assert index['primary'] and index['unique']
                assert index['columns'] == [{'number': 0, 'ascending': True}]
            else:
                assert not index['primary'] and not index['unique']
                assert index['flags'] == PUBLIC_ORDINARY_INDEX_FLAGS
                assert index['columns'] == [{'number': 1, 'ascending': label != 'Descending'}]
            ids = [references[(page << 8) | slot] for page, slot, _ in native]
            assert sorted(ids) == list(range(1, count + 1))
            visited, leaves = tree(pages, index['root'])
            assert leaves == observed
            owned = maps[f'{name}:Index:{label}']['file_members']
            assert sorted(visited) == sorted(owned)
            index['pages'] = visited
            index['height'] = pages[index['root']]['height']
            index['ids'] = ids
            if name == 'LongSentinel' or label == 'PrimaryID':
                assert ids == list(range(1, count + 1))
                continue
            old, new = map(bytes.fromhex, encoders[(name, label)])
            for (_, _, key), identity in zip(native, ids, strict=True):
                expected = new if changed == name and identity == 1 else old
                assert bytes.fromhex(key) == expected
            expected_ids = list(range(1, ROWS + 1))
            if changed == name and label != 'Descending':
                expected_ids = list(range(2, ROWS + 1)) + [1]
            assert ids == expected_ids
            if changed == 'none':
                assert len(visited) == 1 and index['height'] == 0
                actual = pages[index['root']]
                assert PAGE - ENTRY_START - actual['free'] == packed([old] * ROWS)
                assert packed([old] * ROWS) <= CAPACITY
                if label == 'Descending':
                    hypothetical = [new] + [old] * (ROWS - 1)
                else:
                    hypothetical = [old] * (ROWS - 1) + [new]
                full = packed(hypothetical)
                halves = [packed(hypothetical[:ROWS // 2]), packed(hypothetical[ROWS // 2:])]
                assert full > CAPACITY
                index['capacity'] = {
                    'packing_policy': 'pinned public key-only prefix; ROW remains in every entry',
                    'old_key': old.hex(),
                    'new_key': new.hex(),
                    'changed_bytes': full,
                    'half_bytes': halves,
                    'half_fit': [size <= CAPACITY for size in halves],
                    'repeated_demand': any(size > CAPACITY for size in halves),
                }
        if name in TARGETS:
            assert set(table['indexes']) == {'PrimaryID', 'Ascending', 'Twin', 'Descending'}
            assert table['indexes']['Ascending']['physical'] == table['indexes']['Twin']['physical']
            assert table['indexes']['Ascending']['root'] == table['indexes']['Twin']['root']
            assert table['indexes']['Ascending']['physical'] != table['indexes']['Descending']['physical']
            ascending_map = maps[f'{name}:Index:Ascending']
            twin_map = maps[f'{name}:Index:Twin']
            assert ascending_map == twin_map
        else:
            assert set(table['indexes']) == {'PrimaryID'}
    return {
        'profile': profile,
        'file': str(path),
        'bytes': len(raw),
        'sha256': hashlib.sha256(raw).hexdigest(),
        'tables': tables,
        'maps': maps,
        'properties': properties,
        'changed': changed,
    }


def frame(raw, page, slot):
    base = page * PAGE
    assert raw[base:base + 2] == b'\x01\x01'
    count = number(raw, base + 12, 2)
    assert 0 <= slot < count <= 255
    word = number(raw, base + 14 + slot * 2, 2)
    assert word & 0xe000 == 0
    start = base + word
    end = base + PAGE if slot == 0 else base + (number(raw, base + 14 + (slot - 1) * 2, 2) & 0x1fff)
    assert base + 14 + 2 * count <= start < end <= base + PAGE
    return start, end


def tree(pages, root):
    visited = []
    levels = {}

    def walk(number, height):
        assert 0 <= height < 16
        assert number not in visited
        visited.append(number)
        page = pages[number]
        assert page['height'] == height
        levels.setdefault(height, []).append(number)
        entries = list(map(bytes.fromhex, page['entries_hex']))
        if height == 0:
            assert page['kind'] == 4 and page['tail'] == 0
            return entries
        assert page['kind'] == 3
        result = []
        for entry in entries:
            child = walk(int.from_bytes(entry[-4:], 'big'), height - 1)
            assert child and child[-1] == entry[:-4]
            result.extend(child)
        if page['tail']:
            result.extend(walk(page['tail'], height - 1))
        return result

    rows = walk(root, pages[root]['height'])
    for level in levels.values():
        for position, number in enumerate(level):
            page = pages[number]
            assert page['previous'] == (level[position - 1] if position else 0)
            assert page['next'] == (level[position + 1] if position + 1 < len(level) else 0)
    return visited, rows
