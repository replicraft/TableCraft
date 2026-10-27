"""Original typed association and root-leaf preservation model, independent of ACE code."""
from collections import deque

PAGE_BYTES = 4096
SLOT_START = 14
SLOT_BYTES = 2
SLOT_OFFSET_MASK = 0x1FFF
TOMBSTONE_MASK = 0xC000
TABLE_COUNT_OFFSET = 16
TABLE_COUNT_BYTES = 4
INDEX_FREE_OFFSET = 2
INDEX_PREFIX_OFFSET = 24
INDEX_HEIGHT_OFFSET = 26
INDEX_BITMAP_START = 27
INDEX_DATA_START = 480
ROW_REFERENCE_BYTES = 4
ROW_REFERENCE_SLOT_BITS = 8
LEAF_KIND = 4


def parse_rows(text):
    tables = {}
    declarations = []
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            tables[fields[1]] = {
                'count': int(fields[2]),
                'definition': int(fields[3]),
                'rows': {},
                'indexes': {},
            }
            declarations.append(fields[:2] + fields[3:])
        elif tag == 'ROW':
            tables[fields[1]]['rows'][int(fields[2])] = fields[3:]
        elif tag == 'INDEX':
            tables[fields[1]]['indexes'][fields[2]] = {
                'physical': int(fields[3]),
                'root': int(fields[4]),
                'entries': int(fields[5]),
                'owned': int(fields[6]),
            }
            declarations.append(fields[:5] + fields[6:])
        else:
            assert tag in ('COLUMN', 'INDEX_COLUMN', 'RELATION', 'RELATION_COLUMN')
            declarations.append(fields)
    for table in tables.values():
        assert table['count'] == len(table['rows'])
    return tables, declarations


def parse_orders(text):
    result = {}
    for line in text.splitlines():
        fields = line.split('\t')
        key = (fields[1], fields[2])
        if fields[0] == 'INDEX':
            assert key not in result
            result[key] = []
        else:
            assert fields[0] == 'ENTRY'
            result[key].append(int(fields[3]))
    return result


def selected_roots(model, table, seeds):
    selected = {name: set() for name in model['tables']}
    pending = deque((table, seed) for seed in seeds)
    identities = set()
    while pending:
        name, identifier = pending.popleft()
        source = model['tables'][name]
        row = source['rows'][str(identifier)]
        physical = (source['definition'], row['page'], row['slot'])
        if physical in identities:
            continue
        identities.add(physical)
        selected[name].add(identifier)
        for relation in model['relations'].values():
            if relation['parent'] != name:
                continue
            for parent, child in relation['original_matches']:
                if parent == identifier:
                    pending.append((relation['child'], child))
    return selected


def unsigned(raw, offset, width):
    return int.from_bytes(raw[offset:offset + width], 'little')


def payloads(raw, tables):
    result = {}
    for name, table in tables.items():
        result[name] = {}
        for identity, fields in table['rows'].items():
            page, slot = map(int, fields[:2])
            offset = page * PAGE_BYTES + SLOT_START + slot * SLOT_BYTES
            word = unsigned(raw, offset, SLOT_BYTES)
            assert word & ~SLOT_OFFSET_MASK == 0
            start = page * PAGE_BYTES + (word & SLOT_OFFSET_MASK)
            if slot == 0:
                end = (page + 1) * PAGE_BYTES
            else:
                preceding = unsigned(raw, offset - SLOT_BYTES, SLOT_BYTES)
                end = page * PAGE_BYTES + (preceding & SLOT_OFFSET_MASK)
            assert page * PAGE_BYTES <= start < end <= (page + 1) * PAGE_BYTES
            result[name][identity] = (offset, word, start, end)
    return result


def leaf(raw, number, owner):
    start = number * PAGE_BYTES
    page = raw[start:start + PAGE_BYTES]
    assert page[:2] == bytes([LEAF_KIND, 1])
    assert unsigned(page, 4, 4) == owner
    assert page[INDEX_HEIGHT_OFFSET] == 0
    assert not any(page[8:24])
    prefix = unsigned(page, INDEX_PREFIX_OFFSET, SLOT_BYTES)
    ends = []
    for index, byte in enumerate(page[INDEX_BITMAP_START:INDEX_DATA_START]):
        for bit in range(8):
            if byte & (1 << bit):
                ends.append(index * 8 + bit)
    entries = []
    previous = 0
    first = b''
    for end in ends:
        assert previous < end <= PAGE_BYTES - INDEX_DATA_START
        entry = page[INDEX_DATA_START + previous:INDEX_DATA_START + end]
        if previous == 0:
            first = entry
        else:
            assert prefix <= len(first)
            entry = first[:prefix] + entry
        assert len(entry) >= ROW_REFERENCE_BYTES
        entries.append(entry)
        previous = end
    free = unsigned(page, INDEX_FREE_OFFSET, SLOT_BYTES)
    assert free == PAGE_BYTES - INDEX_DATA_START - previous
    return entries, PAGE_BYTES - free


def verify(before, after, original, actual, original_orders, actual_orders, selected):
    assert len(before) == len(after)
    old_tables, old_declarations = original
    new_tables, new_declarations = actual
    assert old_declarations == new_declarations
    assert set(old_tables) == set(new_tables)
    assert set(original_orders) == set(actual_orders)
    records = payloads(before, old_tables)
    allowed = bytearray(len(before))
    audited_roots = set()
    removed_total = 0
    for name, old in old_tables.items():
        current = new_tables[name]
        deleted = selected[name]
        removed_total += len(deleted)
        expected = {identity: row for identity, row in old['rows'].items() if identity not in deleted}
        assert list(current['rows'].items()) == list(expected.items())
        assert current['count'] == len(expected)
        count = old['definition'] * PAGE_BYTES + TABLE_COUNT_OFFSET
        assert unsigned(after, count, TABLE_COUNT_BYTES) == len(expected)
        if deleted:
            allowed[count:count + TABLE_COUNT_BYTES] = bytes([1]) * TABLE_COUNT_BYTES
        deleted_refs = set()
        for identity, (offset, word, start, end) in records[name].items():
            expected_word = word
            if identity in deleted:
                expected_word |= TOMBSTONE_MASK
                allowed[offset:offset + SLOT_BYTES] = bytes([1]) * SLOT_BYTES
                page, slot = map(int, old['rows'][identity][:2])
                deleted_refs.add((page << ROW_REFERENCE_SLOT_BITS) | slot)
            assert unsigned(after, offset, SLOT_BYTES) == expected_word
            assert before[start:end] == after[start:end]
        assert set(old['indexes']) == set(current['indexes'])
        for label, declaration in old['indexes'].items():
            key = (name, label)
            expected_order = [identity for identity in original_orders[key] if identity not in deleted]
            assert actual_orders[key] == expected_order
            assert current['indexes'][label]['entries'] == len(expected_order)
            for field in ('physical', 'root', 'owned'):
                assert current['indexes'][label][field] == declaration[field]
            root = declaration['root']
            if root in audited_roots:
                continue
            audited_roots.add(root)
            old_entries, old_end = leaf(before, root, old['definition'])
            new_entries, new_end = leaf(after, root, old['definition'])
            expected_entries = []
            for entry in old_entries:
                reference = int.from_bytes(entry[-ROW_REFERENCE_BYTES:], 'big')
                if reference not in deleted_refs:
                    expected_entries.append(entry)
            assert new_entries == expected_entries
            if deleted:
                page_start = root * PAGE_BYTES
                for offset in (INDEX_FREE_OFFSET, INDEX_PREFIX_OFFSET):
                    begin = page_start + offset
                    allowed[begin:begin + SLOT_BYTES] = bytes([1]) * SLOT_BYTES
                begin = page_start + INDEX_BITMAP_START
                end = page_start + max(old_end, new_end)
                allowed[begin:end] = bytes([1]) * (end - begin)
    for offset, (old, new) in enumerate(zip(before, after, strict=True)):
        if not allowed[offset]:
            assert old == new, ('changed protected byte', offset, old, new)
    return {
        'deleted_roots': removed_total,
        'original_record_bodies_preserved': sum(len(table) for table in records.values()),
        'distinct_root_leaves_checked': len(audited_roots),
        'complete_typed_rows_locations_and_order': True,
        'logical_alias_and_index_inventories_exact': True,
        'full_original_key_row_entries_filtered_exactly': True,
        'all_other_bytes_including_maps_counters_headers_long_values_unchanged': True,
    }
