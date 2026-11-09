"""Read-only getter and raw membership validation for the qualified reference inputs."""

import hashlib

from support import wire

ALLOWED_PROPERTIES = {'Required', 'AllowZeroLength', 'ValidationRule', 'ValidationText',
                      'DefaultValue', 'Caption', 'Description'}


def parse_tables(path, raw):
    tables = {}
    for line in path.read_text().splitlines():
        fields = line.split('\t')
        kind = fields[0]
        if kind == 'TABLE':
            tables[fields[1]] = {'count': int(fields[2]), 'definition': int(fields[3]),
                                 'columns': {}, 'rows': {}, 'indexes': {},
                                 'column_count': int(fields[4]), 'max_columns': int(fields[5]),
                                 'properties': [], 'children': []}
        elif kind in ['COLUMN', 'ROW', 'INDEX', 'KEYCOLUMN', 'ORDER', 'KEY',
                      'CATALOG', 'ACTIVE', 'RELATIONSHIPS', 'PROPERTY', 'CHILD']:
            table = tables[fields[1]]
            if kind == 'COLUMN':
                table['columns'][fields[2]] = fields[3:]
            elif kind == 'ROW':
                identifier, page, slot = map(int, fields[2:5])
                values = dict(zip(table['columns'], fields[5:]))
                assert len(fields[5:]) == table['column_count']
                payload = wire.row(raw, (page << wire.REFERENCE_SLOT_BITS) | slot)
                assert wire.u16(payload, 0) == table['max_columns']
                table['rows'][identifier] = {'page': page, 'slot': slot, 'values': values,
                                            'payload_sha256': hashlib.sha256(payload).hexdigest()}
            elif kind == 'INDEX':
                table['indexes'][fields[2]] = {
                    'physical': int(fields[3]), 'root': int(fields[4]), 'kind': int(fields[5]),
                    'height': int(fields[6]), 'owned_count': int(fields[7]),
                    'historical': int(fields[8]), 'counter_offset': int(fields[9]),
                    'entries': int(fields[10]), 'unique': fields[11] == 'true',
                    'ignore_nulls': fields[12] == 'true', 'required': fields[13] == 'true',
                    'columns': [], 'order': [], 'keys': [],
                }
            elif kind == 'KEYCOLUMN':
                table['indexes'][fields[2]]['columns'].append((fields[3], fields[4] == 'true'))
            elif kind == 'ORDER':
                table['indexes'][fields[2]]['order'].append(int(fields[3]))
            elif kind == 'KEY':
                table['indexes'][fields[2]]['keys'].append((fields[5], int(fields[3]), int(fields[4])))
            elif kind == 'CATALOG':
                table['catalog'] = {'id': int(fields[2]), 'type': int(fields[3]), 'flags': int(fields[4])}
            elif kind == 'ACTIVE':
                table.setdefault('active', []).append(fields[2:])
            elif kind == 'RELATIONSHIPS':
                table['relationships'] = int(fields[2])
            elif kind == 'PROPERTY':
                table['properties'].append(fields[2:])
            elif kind == 'CHILD':
                table['children'].append(fields[2:])

    for name, table in tables.items():
        assert table['count'] == len(table['rows'])
        assert table['column_count'] == table['max_columns'] == len(table['columns'])
        assert table['catalog']['flags'] == 0 and table['catalog']['type'] == 1
        assert not table['children']
        assert all(active[1:] == ['true', 'NULL'] for active in table['active'])
        assert all(prop[1] in ALLOWED_PROPERTIES for prop in table['properties'])
        identities = {(row['page'], row['slot']): identifier for identifier, row in table['rows'].items()}
        for index in table['indexes'].values():
            columns = [column for column, ascending in index['columns']]
            eligible = {identifier for identifier, row in table['rows'].items()
                        if not index['ignore_nulls'] or
                        not all(row['values'][column] == 'NULL' for column in columns)}
            assert set(index['order']) == eligible
            assert len(index['order']) == len(eligible) == index['entries']
            assert [identities[(page, slot)] for key, page, slot in index['keys']] == index['order']
            usage, root = wire.physical(raw, table['definition'], index['physical'])
            assert root == index['root']
            keys, leaves, nodes = wire.decode_tree(raw, root, table['definition'])
            assert keys == index['keys']
            owned = wire.members(raw, usage)
            assert len(owned) == index['owned_count']
            index.update({'usage': usage, 'owned': owned, 'leaves': leaves, 'nodes': nodes,
                          'eligible_ids': sorted(eligible)})
    return tables


def parse_getters(path, raw):
    tables = parse_tables(path, raw)
    relationships = {}
    native_types = {}
    for line in path.read_text().splitlines():
        fields = line.split('\t')
        if fields[0] == 'ENFORCEMENT':
            assert fields[1] == 'true'
        elif fields[0] == 'RELATION':
            relationships[fields[1]] = {
                'parent': fields[2], 'child': fields[3], 'enforced': fields[4] == 'true',
                'update_cascade': fields[5] == 'true', 'delete_cascade': fields[6] == 'true',
                'delete_set_null': fields[7] == 'true', 'columns': [],
            }
        elif fields[0] == 'RELATION_COLUMN':
            relationships[fields[1]]['columns'].append((fields[3], fields[4]))
        elif fields[0] == 'NATIVE_TYPE':
            native_types[(fields[1], fields[2])] = tuple(map(int, fields[3:]))
    assert len(relationships) == 10
    for relation in relationships.values():
        assert relation['enforced'] and not relation['delete_cascade'] and not relation['delete_set_null']
        assert relation['parent'] != relation['child']
    for name, table in tables.items():
        assert table['relationships'] == sum(name in [r['parent'], r['child']] for r in relationships.values())
    return {'tables': tables, 'relationships': relationships, 'native_types': native_types}
