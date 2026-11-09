"""Independent original-row, map and raw-tree models for batch DELETE evidence."""
import collections
import hashlib
import uuid

import capture_delete_trees as TREE
import capture_delete_contraction as CAP

NODE = CAP.INDEX_NODE_KIND
LEAF = CAP.INDEX_LEAF_KIND
REFERENCE_BYTES = CAP.ROW_REFERENCE_BYTES


def parse(text, original=False):
    result = {}
    relationships = []
    for line in text.splitlines():
        cells = line.split('\t')
        tag, name = cells[:2]
        if tag == 'RELATION':
            relationships.append(cells[1:])
        elif tag == 'TABLE':
            fields = ['live_rows', 'definition_page', 'page_bytes', 'count_offset',
                      'row_start', 'mask_offset', 'mask_bytes']
            result[name] = {
                'geometry': dict(zip(fields, map(int, cells[2:]), strict=True)),
                'columns': [], 'rows': {}, 'locations': {}, 'indexes': {},
            }
        elif tag == 'COLUMN':
            result[name]['columns'].append(cells[2:])
        elif tag == 'LOCATION':
            result[name]['locations'][int(cells[2])] = tuple(map(int, cells[3:]))
        elif tag == 'ROW':
            result[name]['rows'][int(cells[2])] = cells[3:]
        elif tag == 'INDEX':
            result[name]['indexes'][cells[2]] = {
                'physical': int(cells[3]), 'historical_count': int(cells[4]),
                'counter_offset': int(cells[5]), 'entry_count': int(cells[6]),
                'owned_pages': int(cells[7]), 'unique': cells[8],
                'ignore_nulls': cells[9], 'columns': [], 'members': [],
            }
        elif tag == 'INDEX_COLUMN':
            result[name]['indexes'][cells[2]]['columns'].append(cells[3:])
        elif tag == 'ENTRY':
            result[name]['indexes'][cells[2]]['members'].append(int(cells[3]))
        else:
            raise AssertionError('Unexpected getter record')
    assert relationships == [['Parents_Children', 'Parents', 'Children', 'true', 'false', 'false', 'false']]
    assert set(result) == {'Parents', 'Children', 'Sentinel'}
    for name, table in result.items():
        assert len(table['rows']) == table['geometry']['live_rows']
        assert set(table['rows']) == set(table['locations'])
        if name == 'Sentinel':
            continue
        expected_indexes = {'PrimaryID', 'Wide', 'WideAlias'}
        if name == 'Children':
            expected_indexes.update({'ParentAsc', 'ParentAlias', 'Parents_Children'})
        else:
            expected_indexes.add('.rB')
        assert set(table['indexes']) == expected_indexes
        if original:
            assert set(table['rows']) == set(range(1, 501))
        for identity, values in table['rows'].items():
            if name == 'Children':
                assert values[0] == str(identity)
                values = values[1:]
            assert len(values) == 10
            for component, text in enumerate(values):
                seed = f'TableCraft original two-owner deletion/{name}/{identity}/{component}'
                expected = uuid.UUID(bytes=hashlib.sha256(seed.encode()).digest()[:16])
                assert uuid.UUID(text) == expected
        for index in table['indexes'].values():
            assert index['entry_count'] == len(table['rows'])
            assert index['ignore_nulls'] == 'false'
            assert sorted(index['members']) == sorted(table['rows'])
    return result


def geometry(raw, info, map_info, name):
    pages = TREE.pages(raw, info)
    indexes = {}
    for key, value in map_info['indexes'].items():
        if key.startswith(name + '/'):
            indexes[key.split('/', 1)[1]] = value
    references = {}
    for identity, (page, slot) in info['locations'].items():
        references[(page << CAP.ROW_REFERENCE_SLOT_BITS) | slot] = identity
    roots = set(index['root'] for index in indexes.values())
    visited = set()
    trees = []
    for root in sorted(roots):
        levels = collections.defaultdict(list)
        partitions = []
        owned = []

        def walk(number):
            assert number not in visited
            visited.add(number)
            owned.append(number)
            page = pages[number]
            levels[page['height']].append(number)
            entries = [bytes.fromhex(value) for value in page['entries_hex']]
            if page['kind'] == LEAF:
                assert page['height'] == 0 and page['tail'] == 0
                identities = [references[int.from_bytes(value[-REFERENCE_BYTES:], 'big')] for value in entries]
                partitions.append({'page': number, 'ids': identities})
                return entries, identities
            assert page['kind'] == NODE
            flattened = []
            identities = []
            children = CAP.children(page)
            for position, child in enumerate(children):
                keys, ids = walk(child)
                assert keys and pages[child]['height'] == page['height'] - 1
                if position < len(entries):
                    assert entries[position][:-REFERENCE_BYTES] == keys[-1]
                flattened.extend(keys)
                identities.extend(ids)
            return flattened, identities

        keys, identities = walk(root)
        names = []
        for label, metadata in indexes.items():
            if metadata['root'] != root:
                continue
            names.append(label)
            assert info['indexes'][label]['members'] == identities
            assert metadata['owned_count'] == len(owned)
            assert set(map_info['decoded_maps']['Index' + name + str(metadata['physical']) + 'Owned']['file_members']) == set(owned)
        for order in levels.values():
            for position, number in enumerate(order):
                previous = order[position - 1] if position else 0
                following = order[position + 1] if position + 1 < len(order) else 0
                assert (pages[number]['previous'], pages[number]['next']) == (previous, following)
        trees.append({
            'root': root, 'height': pages[root]['height'], 'logical_names': names,
            'pages': sorted(owned), 'levels': dict(levels), 'leaf_partitions': partitions,
            'key_count': len(keys),
            'full_key_order_sha256': hashlib.sha256(b''.join(keys)).hexdigest(),
        })
    assert visited == set(pages)
    assert indexes['Wide']['physical'] == indexes['WideAlias']['physical']
    return pages, trees
