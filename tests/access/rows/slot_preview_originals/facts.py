"""Read-only typed getter parsing and independent ordinary-record/index geometry."""
import capture_delete_trees as TREE
from . import common

PAGE_BYTES = 4096
COLUMN_BLOCK = 63
COUNTER_BYTES = 12
COLUMN_BYTES = 25
PHYSICAL_COUNT = 51
COLUMN_FLAGS = 15
COLUMN_EXTENDED_FLAGS = 16
SLOT_START = 14
SLOT_BYTES = 2
SLOT_MASK = 0x1FFF
VARIABLE_BYTES = 2
NODE_KIND = 3
LEAF_KIND = 4
CHILD_BYTES = 4
MASK_BITS = 8
REFERENCE_SLOT_BITS = 8
ENTRY_MAP_OFFSET = 27
ENTRY_MAP_BYTES = 453

def number(data, offset, width=4):
    return int.from_bytes(data[offset:offset + width], 'little')


def parse(text):
    tables = {}
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            tables[fields[1]] = {
                'count': int(fields[2]),
                'definition': int(fields[3]),
                'columns_count': int(fields[4]),
                'max_columns': int(fields[5]),
                'columns': {},
                'properties': [],
                'rows': {},
                'raw_rows': {},
                'indexes': {},
            }
        elif tag == 'COLUMN':
            names = ['position', 'number', 'native_type', 'declared_bytes']
            column = dict(zip(names, map(int, fields[3:7]), strict=True))
            column['compressed'] = fields[7] == 'true'
            column['variable'] = int(fields[8])
            column['sort'] = {'lcid': int(fields[9]), 'variant': int(fields[10]), 'version': int(fields[11])}
            tables[fields[1]]['columns'][fields[2]] = column
        elif tag == 'PROPERTY':
            tables[fields[1]]['properties'].append(fields[2:])
        elif tag == 'ROW':
            tables[fields[1]]['rows'][int(fields[2])] = {
                'page': int(fields[3]),
                'slot': int(fields[4]),
                'values': fields[5:],
            }
        elif tag == 'RAWROW':
            tables[fields[1]]['raw_rows'][int(fields[2])] = fields[3]
        elif tag == 'INDEX':
            tables[fields[1]]['indexes'][fields[2]] = {
                'physical': int(fields[3]),
                'root': int(fields[4]),
                'entries': int(fields[5]),
                'owned_pages': int(fields[6]),
                'primary': fields[7] == 'true',
                'unique': fields[8] == 'true',
                'required': fields[9] == 'true',
                'ignored_null': fields[10] == 'true',
                'columns': [],
                'order': [],
            }
        elif tag == 'INDEX_COLUMN':
            tables[fields[1]]['indexes'][fields[2]]['columns'].append(fields[3:])
        elif tag == 'ENTRY':
            tables[fields[1]]['indexes'][fields[2]]['order'].append(int(fields[3]))
        else:
            raise AssertionError(tag)
    return tables


def inspect(profile, path, getter):
    raw = path.read_bytes()
    tables = parse(getter.read_text())
    flags = {}
    for name, table in tables.items():
        assert table['count'] == len(table['rows']) == len(table['raw_rows'])
        definition = raw[table['definition'] * PAGE_BYTES:(table['definition'] + 1) * PAGE_BYTES]
        physical = number(definition, PHYSICAL_COUNT)
        columns = COLUMN_BLOCK + physical * COUNTER_BYTES
        for label, column in table['columns'].items():
            field = columns + column['position'] * COLUMN_BYTES
            column['flags'] = definition[field + COLUMN_FLAGS]
            column['extended_flags'] = definition[field + COLUMN_EXTENDED_FLAGS]
            flags.setdefault(label, set()).add((column['flags'], column['extended_flags']))
        references = {}
        for identity, row in table['rows'].items():
            body = bytes.fromhex(table['raw_rows'][identity])
            offset = row['page'] * PAGE_BYTES + SLOT_START + row['slot'] * SLOT_BYTES
            word = number(raw, offset, SLOT_BYTES)
            assert word & ~SLOT_MASK == 0
            start = row['page'] * PAGE_BYTES + (word & SLOT_MASK)
            if row['slot'] == 0:
                end = (row['page'] + 1) * PAGE_BYTES
            else:
                end = row['page'] * PAGE_BYTES + (number(raw, offset - SLOT_BYTES, SLOT_BYTES) & SLOT_MASK)
            assert raw[start:end] == body
            assert number(body, 0, SLOT_BYTES) == table['max_columns']
            mask = len(body) - (table['max_columns'] + MASK_BITS - 1) // MASK_BITS
            count = number(body, mask - VARIABLE_BYTES, VARIABLE_BYTES)
            row['slots'] = {}
            for label, column in table['columns'].items():
                if column['native_type'] not in (9, 10):
                    continue
                variable = column['variable']
                assert variable < count
                first = mask - VARIABLE_BYTES - VARIABLE_BYTES * (variable + 1)
                second = mask - VARIABLE_BYTES - VARIABLE_BYTES * (variable + 2)
                begin = number(body, first, VARIABLE_BYTES)
                stop = number(body, second, VARIABLE_BYTES)
                assert begin <= stop <= mask - VARIABLE_BYTES * (count + 2)
                present = bool(body[mask + column['number'] // MASK_BITS] & 1 << column['number'] % MASK_BITS)
                row['slots'][label] = {
                    'absolute_start': start + begin,
                    'absolute_end': start + stop,
                    'row_start': begin,
                    'row_end': stop,
                    'byte_width': stop - begin,
                    'present': present,
                    'wire_hex': body[begin:stop].hex(),
                }
            references[row['page'] << REFERENCE_SLOT_BITS | row['slot']] = identity
        geometry = {'geometry': {
            'page_bytes': PAGE_BYTES, 'mask_offset': ENTRY_MAP_OFFSET,
            'mask_bytes': ENTRY_MAP_BYTES, 'definition_page': table['definition'],
        }}
        pages = TREE.pages(raw, geometry)
        table['physical_trees'] = {}
        for index in table['indexes'].values():
            root = index['root']
            if root in table['physical_trees']:
                continue
            visited = []

            def visit(page):
                assert page not in visited
                visited.append(page)
                current = pages[page]
                entries = [bytes.fromhex(value) for value in current['entries_hex']]
                if current['kind'] == LEAF_KIND:
                    return [references[int.from_bytes(entry[-CHILD_BYTES:], 'big')] for entry in entries]
                assert current['kind'] == NODE_KIND
                result = []
                for entry in entries:
                    result.extend(visit(int.from_bytes(entry[-CHILD_BYTES:], 'big')))
                if current['tail']:
                    result.extend(visit(current['tail']))
                return result
            order = visit(root)
            assert order == index['order']
            assert len(order) == index['entries']
            assert len(visited) == index['owned_pages']
            table['physical_trees'][root] = {
                'height': pages[root]['height'],
                'pages': visited,
                'node_count': sum((pages[page]['kind'] == NODE_KIND for page in visited)),
                'leaf_count': sum((pages[page]['kind'] == LEAF_KIND for page in visited)),
                'root_sha256': pages[root]['sha256'],
            }
        for label, index in table['indexes'].items():
            assert len(index['order']) == index['entries']
            if not index['ignored_null']:
                assert set(index['order']) == set(table['rows'])
        if name.startswith('Indexed'):
            for first, alias in [('TextAsc', 'TextAlias'), ('BytesAsc', 'BytesAlias')]:
                assert table['indexes'][first]['physical'] == table['indexes'][alias]['physical']
    return {
        'profile': profile,
        'input': common.pin(path),
        'getter': common.pin(getter),
        'tables': tables,
        'flags': {key: sorted(value) for key, value in flags.items()},
    }
