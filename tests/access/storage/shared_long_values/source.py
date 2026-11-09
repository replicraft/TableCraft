"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def parse(
    text,
    *,
    label,
):
    tables = {}
    maps = {}
    absent = []
    properties = {}
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            name = label(fields[1])
            assert name not in tables
            tables[name] = dict(zip(('definition', 'count', 'columns_count', 'max_columns'),
                                    map(int, fields[2:]), strict=True),
                                columns={}, indexes={}, rows={}, values={})
        elif tag == 'COLUMN':
            table, name = label(fields[1]), label(fields[2])
            assert name not in tables[table]['columns']
            assert fields[7] in ('true', 'false') and fields[8] in ('true', 'false')
            tables[table]['columns'][name] = {
                'position': int(fields[3]), 'number': int(fields[4]),
                'native_type': int(fields[5]), 'length': int(fields[6]),
                'auto': fields[7] == 'true', 'compressed': fields[8] == 'true',
                'variable': int(fields[9]),
            }
        elif tag == 'PROPERTY':
            properties.setdefault(fields[1], []).append([
                label(fields[2]), int(fields[3]), fields[4], fields[5],
                None if fields[6] == 'NULL' else label(fields[6]),
            ])
        elif tag == 'MAP':
            role = fields[1]
            assert role not in maps
            maps[role] = dict(zip(('table_page', 'row_start', 'row_end', 'start_page',
                                   'end_page', 'reported_count'), map(int, fields[2:]),
                                  strict=True), all_members=[])
        elif tag == 'MEMBER':
            maps[fields[1]]['all_members'].append(int(fields[2]))
        elif tag == 'MAP_ABSENT':
            assert len(fields) == 3 and fields[2] in ('NULL', 'LEGACY')
            assert all(item['role'] != fields[1] for item in absent)
            absent.append({'role': fields[1], 'reason': fields[2]})
        elif tag == 'INDEX':
            table, name = label(fields[1]), label(fields[2])
            assert name not in tables[table]['indexes'] and fields[9] in ('true', 'false')
            tables[table]['indexes'][name] = dict(zip(
                ('physical', 'root', 'entries', 'history', 'history_offset', 'flags'),
                map(int, fields[3:9]), strict=True), primary=fields[9] == 'true',
                columns=[], rows=[])
        elif tag == 'KEY_COLUMN':
            table, index, column = map(label, fields[1:4])
            assert fields[4] in ('true', 'false')
            tables[table]['indexes'][index]['columns'].append((column, fields[4] == 'true'))
        elif tag == 'ENTRY':
            index = tables['Items']['indexes'][label(fields[1])]
            index['rows'].append(tuple(map(int, fields[2:])))
        elif tag == 'ROW':
            table, identity = label(fields[1]), int(fields[2])
            assert identity not in tables[table]['rows']
            tables[table]['rows'][identity] = {
                'page': int(fields[3]), 'slot': int(fields[4]),
                'target_page': int(fields[5]), 'target_slot': int(fields[6]),
                'raw': bytes.fromhex(fields[7]),
            }
            tables[table]['values'][identity] = {}
        elif tag == 'VALUE':
            table, identity, column = label(fields[1]), int(fields[2]), label(fields[3])
            values = tables[table]['values'][identity]
            assert column not in values
            carrier, kind, encoded = fields[4:]
            if kind == 'NULL':
                assert carrier == 'null' and encoded == 'NULL'
                value = None
            elif kind == 'INT32':
                assert carrier == 'java.lang.Integer'
                value = int(encoded)
            elif kind == 'BINARY':
                assert carrier == '[B'
                value = bytes.fromhex(encoded)
            else:
                assert kind == 'TEXT' and carrier == 'java.lang.String'
                value = label(encoded)
            values[column] = value
        else:
            raise AssertionError(('Unexpected frozen getter tag', tag))
    return tables, maps, properties, absent


def inventory(
    raw,
    owned,
    referenced,
    *,
    PAGE,
    number,
):
    result = {'live': [], 'deleted': [], 'orphan': [], 'pages': []}
    for page in owned:
        base = page * PAGE
        body = raw[base:base + PAGE]
        assert body[:2] == b'\x01\x01' and body[4:8] == b'LVAL'
        count = number(body, 12)
        assert count <= 255
        previous = PAGE
        for slot in range(count):
            word = number(body, 14 + slot * 2)
            assert word & 0x2000 == 0
            start = word & 0x1fff
            assert 14 + count * 2 <= start <= previous <= PAGE
            reference = page << 8 | slot
            item = {'reference': reference, 'word': word, 'start': base + start,
                    'end': base + previous}
            if word & 0x8000:
                assert reference not in referenced
                result['deleted'].append(item)
            else:
                assert word & 0x4000 == 0 and start < previous
                result['live'].append(item)
                if reference not in referenced:
                    result['orphan'].append(item)
            previous = start
        assert number(body, 2) == previous - (14 + count * 2)
        result['pages'].append(page)
    assert referenced <= {item['reference'] for item in result['live']}
    return result
