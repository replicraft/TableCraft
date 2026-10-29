"""Complete DELETE/SET NULL images from original state and explicit row seeds only."""
import hashlib

PAGE_BYTES = 4096
TABLE_ROWS_OFFSET = 16
SLOT_DIRECTORY_OFFSET = 14
SLOT_BYTES = 2
SLOT_OFFSET_MASK = 0x1fff
DELETED_FLAGS = 0xc000
ENTRY_START = 480
ENTRY_BITMAP_START = 27
ENTRY_CAPACITY = PAGE_BYTES - ENTRY_START
PREFIX_OFFSET = 24
FREE_OFFSET = 2
ROW_REFERENCE_BYTES = 4
COLUMN_COUNT_BYTES = 2


def number(raw, offset, width):
    part = raw[offset:offset + width]
    assert len(part) == width
    return int.from_bytes(part, 'little')


def put(raw, offset, value, width):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def key(value):
    if value is None:
        return b'\x00'
    assert type(value) is int and -(1 << 31) <= value < (1 << 31)
    ordered = (value & 0xffffffff) ^ 0x80000000
    return b'\x7f' + ordered.to_bytes(4, 'big')


def leaf(original, entries):
    assert len(original) == PAGE_BYTES and original[:2] == b'\x04\x01'
    assert original[26] == 0 and original[8:24] == bytes(16)
    assert entries and entries == sorted(entries, key=lambda entry: (entry[:-4], entry[-4:]))
    keys = [entry[:-ROW_REFERENCE_BYTES] for entry in entries]
    common = min(map(len, keys))
    for candidate in keys[1:]:
        limit = 0
        while limit < common and keys[0][limit] == candidate[limit]:
            limit += 1
        common = limit
    # The admitted cases retain at least two members in every changed tree.
    assert len(entries) >= 2
    prefix = min(number(original, PREFIX_OFFSET, 2), common)
    encoded_length = sum(map(len, entries)) - prefix * (len(entries) - 1)
    if encoded_length > ENTRY_CAPACITY:
        prefix = common
        encoded_length = sum(map(len, entries)) - prefix * (len(entries) - 1)
    assert encoded_length <= ENTRY_CAPACITY
    result = bytearray(original)
    result[ENTRY_BITMAP_START:ENTRY_START] = bytes(ENTRY_START - ENTRY_BITMAP_START)
    end = 0
    for ordinal, entry in enumerate(entries):
        suffix = entry if ordinal == 0 else entry[prefix:]
        result[ENTRY_START + end:ENTRY_START + end + len(suffix)] = suffix
        end += len(suffix)
        result[ENTRY_BITMAP_START + end // 8] |= 1 << (end % 8)
    assert end == encoded_length
    put(result, FREE_OFFSET, ENTRY_CAPACITY - end, 2)
    put(result, PREFIX_OFFSET, prefix, 2)
    return bytes(result)


def original_associations(state):
    result = []
    for declaration in state['relationships']:
        assert declaration['ccolumn'] == 1 and declaration['icolumn'] == 0
        assert declaration['grbit'] in (0x1000, 0x2000)
        parent = declaration['szReferencedObject']
        child = declaration['szObject']
        parent_column = declaration['szReferencedColumn']
        child_column = declaration['szColumn']
        assert parent_column == 'ID' and child_column == 'ParentID'
        matches = []
        for parent_id, parent_values in state['tables'][parent]['values'].items():
            parent_value = parent_values[parent_column]
            assert parent_value is not None
            for child_id, child_values in state['tables'][child]['values'].items():
                if child_values[child_column] is not None and child_values[child_column] == parent_value:
                    matches.append((parent_id, child_id))
        result.append(dict(parent=parent, child=child, column=child_column,
                           cascade=declaration['grbit'] == 0x1000, matches=matches))
    return result


def derive(state, requests):
    before = state['raw']
    after = bytearray(before)
    tables = state['tables']
    associations = original_associations(state)
    deleted = {name: set() for name in tables}
    pending = []
    for name, identity in requests:
        assert name == 'Parents' and type(identity) is int
        assert identity in tables[name]['rows']
        assert (name, identity) not in pending
        pending.append((name, identity))
    while pending:
        name, identity = pending.pop(0)
        if identity in deleted[name]:
            continue
        deleted[name].add(identity)
        for relation in associations:
            if relation['cascade'] and relation['parent'] == name:
                pending.extend((relation['child'], child_id)
                               for parent_id, child_id in relation['matches'] if parent_id == identity)
    nulled = set()
    for relation in associations:
        if relation['cascade']:
            continue
        for parent_id, child_id in relation['matches']:
            if parent_id in deleted[relation['parent']] and child_id not in deleted[relation['child']]:
                nulled.add((relation['child'], child_id, relation['column']))
    final_values = {}
    for name, table in tables.items():
        final_values[name] = {}
        for identity, values in table['values'].items():
            if identity not in deleted[name]:
                final_values[name][identity] = dict(values)

    for name, identity, column in nulled:
        assert final_values[name][identity][column] is not None
        final_values[name][identity][column] = None
    touched = {name for name, rows in deleted.items() if rows}
    touched.update(name for name, identity, column in nulled)
    changed_offsets = set()
    physical_changes = []
    for name in sorted(touched):
        table = tables[name]
        for row in table['rows'].values():
            assert (row['page'], row['slot']) == (row['target_page'], row['target_slot'])
        definition = table['definition']
        if deleted[name]:
            count_offset = definition * PAGE_BYTES + TABLE_ROWS_OFFSET
            put(after, count_offset, table['count'] - len(deleted[name]), 4)
            changed_offsets.update(range(count_offset, count_offset + 4))
        for identity in sorted(deleted[name]):
            row = table['rows'][identity]
            offset = row['page'] * PAGE_BYTES + SLOT_DIRECTORY_OFFSET + row['slot'] * SLOT_BYTES
            word = number(before, offset, SLOT_BYTES)
            assert word & ~SLOT_OFFSET_MASK == 0
            put(after, offset, word | DELETED_FLAGS, SLOT_BYTES)
            changed_offsets.update(range(offset, offset + SLOT_BYTES))
        for table_name, identity, column in sorted(nulled):
            if table_name != name:
                continue
            row = table['rows'][identity]
            assert (row['page'], row['slot']) == (row['target_page'], row['target_slot'])
            columns = number(row['raw'], 0, COLUMN_COUNT_BYTES)
            position = table['columns'][column]['number']
            assert position < columns
            start = number(before, row['page'] * PAGE_BYTES + SLOT_DIRECTORY_OFFSET + row['slot'] * SLOT_BYTES, SLOT_BYTES)
            mask_offset = row['page'] * PAGE_BYTES + start + len(row['raw']) - (columns + 7) // 8
            bit = 1 << (position % 8)
            assert before[mask_offset + position // 8] & bit
            after[mask_offset + position // 8] &= ~bit
            changed_offsets.add(mask_offset + position // 8)
        seen = set()
        for index_name, index in table['indexes'].items():
            physical = index['physical']
            if physical in seen:
                continue
            seen.add(physical)
            assert len(index['columns']) == 1 and index['columns'][0][1] is True
            column = index['columns'][0][0]
            entries = []
            original_entries = []
            for identity, values in table['values'].items():
                row = table['rows'][identity]
                reference = row['page'] << 8 | row['slot']
                value = values[column]
                if value is not None or not index['flags'] & 2:
                    original_entries.append(key(value) + reference.to_bytes(ROW_REFERENCE_BYTES, 'big'))
                if identity not in final_values[name]:
                    continue
                value = final_values[name][identity][column]
                assert value is not None or not index['flags'] & 8
                if value is not None or not index['flags'] & 2:
                    entries.append(key(value) + reference.to_bytes(ROW_REFERENCE_BYTES, 'big'))
            entries.sort(key=lambda entry: (entry[:-4], entry[-4:]))
            original_entries.sort(key=lambda entry: (entry[:-4], entry[-4:]))
            if entries == original_entries:
                continue
            base = index['root'] * PAGE_BYTES
            original = before[base:base + PAGE_BYTES]
            rendered = leaf(original, entries)
            after[base:base + PAGE_BYTES] = rendered
            changed_offsets.update(offset for offset in range(base, base + PAGE_BYTES)
                                   if before[offset] != after[offset])
            physical_changes.append(dict(
                table=name,
                physical=physical,
                root=index['root'],
                source_entries=len(original_entries),
                final_entries=len(entries),
                history=index['history'],
                prefix=number(rendered, PREFIX_OFFSET, 2),
            ))
    assert len(after) == len(before)
    assert all(old == new or offset in changed_offsets
               for offset, (old, new) in enumerate(zip(before, after, strict=True)))
    # All full original bodies survive, except exactly the independently located mask cells.
    for name in touched:
        table = tables[name]
        for identity, row in tables[name]['rows'].items():
            directory = row['page'] * PAGE_BYTES + SLOT_DIRECTORY_OFFSET + row['slot'] * SLOT_BYTES
            start_word = number(before, directory, SLOT_BYTES)
            assert start_word & ~SLOT_OFFSET_MASK == 0
            start = row['page'] * PAGE_BYTES + start_word
            body = bytearray(row['raw'])
            for table_name, target, column in nulled:
                if table_name == name and target == identity:
                    position = table['columns'][column]['number']
                    columns = number(body, 0, COLUMN_COUNT_BYTES)
                    mask = len(body) - (columns + 7) // 8
                    body[mask + position // 8] &= ~(1 << (position % 8))
            assert after[start:start + len(body)] == body
    details = dict(source_sha256=hashlib.sha256(before).hexdigest(),
                   expected_sha256=hashlib.sha256(after).hexdigest(),
                   deleted={name:sorted(rows) for name, rows in deleted.items() if rows},
                   nulled=[list(item) for item in sorted(nulled)],
                   final_rows={name:values for name, values in final_values.items() if name in touched},
                   physical_changes=physical_changes, changed_bytes=sum(a != b for a, b in zip(before, after, strict=True)),
                   allocation_changes=0, history_changes=0, original_fixed_payloads_preserved=True)
    return bytes(after), details
