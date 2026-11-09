"""Raw read-back certification of prospective images, never a producer input."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_fixed_ignore_null_updates import source as raw_reader

PAGE = 4096


def integer(raw, offset, width=4):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


def tree(raw, root, owner, columns):
    pages = set()
    levels = {}
    def visit(number, expected_height):
        assert number not in pages and 0 < number < len(raw) // PAGE
        pages.add(number)
        block = raw[number * PAGE:(number + 1) * PAGE]
        assert block[1] == 1 and integer(block, 4) == owner
        height = block[26]
        assert height == expected_height and block[0] == (4 if height == 0 else 3)
        prefix = integer(block, 24, 2)
        assert prefix <= 5 * len(columns) + 4
        assert not block[27] & 1 and not block[479] & 0xfe
        ends = [position for position in range(1, 3617) if block[27 + position // 8] & (1 << (position % 8))]
        assert (ends[-1] if ends else 0) == 3616 - integer(block, 2, 2)
        entries = []
        children = []
        start = 0
        for end in ends:
            assert start < end
            payload = block[480 + start:480 + end]
            if height:
                assert len(payload) >= 4
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            complete = payload if not entries else entries[-1][:prefix] + payload
            raw_reader.decode_entry(complete, columns)
            if entries:
                assert (entries[-1][:-4], entries[-1][-4:]) < (complete[:-4], complete[-4:])
            entries.append(complete)
            start = end
        levels.setdefault(height, []).append(number)
        tail = integer(block, 20)
        if not height:
            assert tail == 0
            return entries
        assert tail != 0
        children.append(tail)
        result = []
        for index, child in enumerate(children):
            values = visit(child, height - 1)
            assert values
            if index < len(entries):
                assert entries[index] == values[-1]
            result.extend(values)
        assert all((a[:-4], a[-4:]) < (b[:-4], b[-4:]) for a,b in zip(result, result[1:]))
        return result
    entries = visit(root, raw[root * PAGE + 26])
    for sequence in levels.values():
        for position, number in enumerate(sequence):
            offset = number * PAGE
            assert integer(raw, offset + 12) == (sequence[position - 1] if position else 0)
            assert integer(raw, offset + 16) == (sequence[position + 1] if position + 1 < len(sequence) else 0)
    return pages, [raw_reader.decode_entry(entry, columns) for entry in entries]


def certify(case, before, image, details):
    tables = before['facts']['tables']
    selected = case['table']
    expected = {row['id']: dict(row['values']) for row in tables[selected]['rows']}
    for identity, values in case['updates']:
        expected[identity].update(values)
    row_count = 0
    for name, table in tables.items():
        declarations = table['columns']
        for row in table['rows']:
            root = row['root']
            page, slot = root >> 8, root & 255
            base = page * PAGE
            start = integer(image, base + 14 + slot * 2, 2) & 0x1fff
            end = PAGE if slot == 0 else integer(image, base + 12 + slot * 2, 2) & 0x1fff
            body = image[base + start:base + end]
            values = {column['name']: raw_reader.records.scalar_value(body, column) for column in declarations}
            assert values == (expected[row['id']] if name == selected else row['values'])
            assert len(body) == len(bytes.fromhex(row['raw_hex']))
            row_count += 1
    table = tables[selected]
    all_new = set()
    for physical, index_name in [(0,'LeftOrder'),(1,'PairOrder')]:
        index = table['indexes'][index_name]
        pages, decoded = tree(image, index['root'], table['definition'], index['columns'])
        wanted = {(tuple(expected[row['id']][column] for column, _ in index['columns']), row['root'])
                  for row in table['rows']
                  if not (index['flags'] & 2 and all(expected[row['id']][column] is None for column,_ in index['columns']))}
        assert set(decoded) == wanted and len(decoded) == len(wanted)
        assert pages == set(details['physical'][physical]['members'])
        for number in pages - {index['root']}:
            assert number not in all_new
            all_new.add(number)
    eof = len(before['raw']) // PAGE
    assert all_new == set(range(eof, len(image) // PAGE))
    checked_maps = 0
    selected_owner = 'Table:' + str(table['definition']) + ':IndexData:'
    for role, mapping in before['facts']['maps'].items():
        wanted_members = set(mapping['file_members'])
        delta = 0
        if role == 'GlobalFree':
            delta = -len(all_new)
        elif role.startswith(selected_owner):
            physical = int(role.removeprefix(selected_owner))
            members = set(details['physical'][physical]['members'])
            wanted_members = members
            delta = len(members) - len(mapping['file_members'])
        page = mapping['map_page']
        descriptor = {'table_page': page, 'row_start': mapping['row_start'] - page * PAGE,
                      'row_end': mapping['row_end'] - page * PAGE,
                      'reported_count': mapping['complete_set_bit_count'] + delta,
                      'file_members': sorted(wanted_members)}
        decoded = raw_reader.native.map_wire.decode_map(image, descriptor, PAGE, len(image) // PAGE)
        assert decoded['payload_type'] == mapping['payload_type']
        assert decoded['row_start'] == mapping['row_start'] and decoded['row_end'] == mapping['row_end']
        checked_maps += 1
    return {'typed_rows': row_count, 'physical_trees': 2, 'new_INDEX_pages': sorted(all_new),
            'maps_checked': checked_maps}
