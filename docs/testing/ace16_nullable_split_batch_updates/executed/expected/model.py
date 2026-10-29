"""Whole engineering images from qualified originals and immutable requests only."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_multiple_indexes import model as frames

PAGE = 4096
CAPACITY = 3616
ENTRY_START = 480


def signed(ordinal):
    assert 0 <= ordinal < 1000
    return -2147483648 + ordinal * 17 if ordinal % 2 == 0 else 2147483647 - ordinal * 17


def requests(case):
    table, recipe = case['table'], case['recipe']
    assert table in ('Items', 'UniqueItems')
    if recipe == 'DuplicateFamily':
        assert table == 'Items'
        return [[identity, {'Group': 42, 'Score': 777}] for identity in (1, 2)]
    assert recipe in ('Full', 'Clear')
    return [[ordinal + 1, {
        'Group': None if recipe == 'Clear' and ordinal >= 300 else signed(ordinal),
        'Score': None if recipe == 'Clear' and ordinal >= 300 else signed(ordinal + 500),
    }] for ordinal in range(500)]


def component(value, ascending):
    if value is None:
        encoded = b'\x00'
    else:
        assert type(value) is int and -2147483648 <= value <= 2147483647
        encoded = b'\x7f' + (value + 2147483648).to_bytes(4, 'big')
    return encoded if ascending else bytes(255 - value for value in encoded)


def key(values, columns):
    return b''.join(component(values[name], ascending) for name, ascending in columns)


def entries(rows, columns, ignored):
    result = []
    for row in rows:
        if ignored and all(row['values'][name] is None for name, _ in columns):
            continue
        result.append((key(row['values'], columns), row['root']))
    result.sort(key=lambda item: (item[0], item[1]))
    return [wire + root.to_bytes(4, 'big') for wire, root in result]


def retained_leaf(original, complete, seed, owner):
    result = frames.leaf(original, complete, seed, owner)
    # The primitive writes a canonical header. Preserve every source-owned byte
    # outside the free/prefix/entry-mask spans of this retained root.
    for offset in range(ENTRY_START):
        allowed = 2 <= offset < 4 or 24 <= offset < 26 or 27 <= offset < ENTRY_START
        if not allowed:
            result[offset] = original[offset]
    return result


def grown_root(original, bound, left, right, owner):
    result = frames.root(original, bound, left, right, owner)
    for offset in range(ENTRY_START):
        allowed = offset == 0 or 2 <= offset < 4 or 20 <= offset < ENTRY_START
        if not allowed:
            result[offset] = original[offset]
    return result


def record_range(raw, root, owner):
    page_number, slot = root >> 8, root & 255
    page = raw[page_number * PAGE:(page_number + 1) * PAGE]
    assert page[:2] == b'\x01\x01' and int.from_bytes(page[4:8], 'little') == owner
    count = int.from_bytes(page[12:14], 'little')
    assert slot < count <= 255
    word = int.from_bytes(page[14 + slot * 2:16 + slot * 2], 'little')
    assert word & 0xe000 == 0
    start = word & 0x1fff
    end = PAGE if slot == 0 else int.from_bytes(page[12 + slot * 2:14 + slot * 2], 'little') & 0x1fff
    assert 14 + count * 2 <= start < end <= PAGE
    return page_number * PAGE + start, page_number * PAGE + end


def map_cell(raw, metadata, page):
    row = raw[metadata['row_start']:metadata['row_end']]
    if row[0] == 0:
        base = int.from_bytes(row[1:5], 'little')
        relative = page - base
        assert 0 <= relative < (len(row) - 5) * 8
        offset = metadata['row_start'] + 5 + relative // 8
    else:
        assert row[0] == 1
        block, relative = divmod(page, (PAGE - 4) * 8)
        position = 1 + 4 * block
        assert position + 4 <= len(row)
        bitmap = int.from_bytes(row[position:position + 4], 'little')
        assert 0 < bitmap < len(raw) // PAGE
        assert raw[bitmap * PAGE:bitmap * PAGE + 4] == b'\x05\x01\x00\x00'
        offset = bitmap * PAGE + 4 + relative // 8
    return offset, 1 << (relative % 8)


def history(original, final, columns, ignored):
    if not ignored:
        return 0
    pairs = [(tuple(before['values'][name] for name, _ in columns),
              tuple(after['values'][name] for name, _ in columns))
             for before, after in zip(original, final, strict=True)]
    null = lambda values: all(value is None for value in values)
    old_families = {before for before, _ in pairs if not null(before)}
    introduced = set()
    for before, after in pairs:
        if not null(before) or null(after):
            continue
        existed = after in old_families
        retained = any(old == new == after for old, new in pairs)
        replacement = any(not null(old) and new == after for old, new in pairs)
        assert not (existed and not retained or not existed and replacement), 'Ambiguous history recipe'
        if not existed:
            introduced.add(after)
    return len(introduced)


def derive(case, before, supplied_requests):
    assert supplied_requests == requests(case)
    original_raw = before['raw']
    facts = before['facts']
    table = facts['tables'][case['table']]
    owner = table['definition']
    assert table['records'] == 500
    original = table['rows']
    assert len({row['root'] for row in original}) == 500
    final = [{'id': row['id'], 'root': row['root'], 'values': dict(row['values'])} for row in original]
    by_id = {row['id']: row for row in final}
    original_by_id = {row['id']: row for row in original}
    ordinals = {row['id']: ordinal for ordinal, row in enumerate(original)}
    assert len(by_id) == 500
    assignments = []
    changed_roots = set()
    field_offsets = set()
    raw = bytearray(original_raw)
    declarations = {column['name']: column for column in table['columns']}
    seen = set()
    for identity, values in supplied_requests:
        assert identity not in seen and identity in by_id and set(values) == {'Group', 'Score'}
        seen.add(identity)
        row = by_id[identity]
        start, end = record_range(original_raw, row['root'], owner)
        assert original_raw[start:end].hex() == original_by_id[identity]['raw_hex']
        assert end - start == 15 and original_raw[start:start + 2] == b'\x03\x00'
        mask = end - 1
        for name in ('Group', 'Score'):
            value = values[name]
            assert value is None or type(value) is int and -2147483648 <= value <= 2147483647
            declaration = declarations[name]
            metadata = bytes.fromhex(declaration['raw_hex'])
            number = declaration['number']
            offset = start + 2 + int.from_bytes(metadata[21:23], 'little')
            bit = 1 << number
            previous = original_by_id[identity]['values'][name]
            assert bool(original_raw[mask] & bit) == (previous is not None)
            if previous is not None:
                assert original_raw[offset:offset + 4] == previous.to_bytes(4, 'little', signed=True)
            assignments.append({'table': case['table'], 'column': name, 'ordinal': ordinals[identity],
                                'identity': identity, 'logical_reference': row['root'],
                                'expected': previous, 'value': value})
            if previous != value:
                changed_roots.add(row['root'])
                if value is not None:
                    raw[offset:offset + 4] = value.to_bytes(4, 'little', signed=True)
                    field_offsets.update(range(offset, offset + 4))
                raw[mask] = (raw[mask] & ~bit) | (bit if value is not None else 0)
                field_offsets.add(mask)
            row['values'][name] = value
    allocations = []
    physical_details = []
    allowed = set(field_offsets)
    map_offsets = set()
    for physical, index_name in [(0, 'LeftOrder'), (1, 'PairOrder')]:
        index = table['indexes'][index_name]
        assert index['physical'] == physical
        ignored = bool(index['flags'] & 2)
        complete = entries(final, index['columns'], ignored)
        if index['flags'] & 1:
            families = [key(row['values'], index['columns']) for row in final
                        if not all(row['values'][name] is None for name, _ in index['columns'])]
            assert len(families) == len(set(families))
        root = index['root']
        source_page = original_raw[root * PAGE:(root + 1) * PAGE]
        assert source_page.hex() == index['framing']['raw_hex']
        seed = int.from_bytes(source_page[24:26], 'little')
        prefix, active = frames.framing(complete, seed)
        role = 'Table:' + str(owner) + ':IndexData:' + str(physical)
        mapping = facts['maps'][role]
        assert mapping['file_members'] == [root]
        if active <= CAPACITY:
            after_page = retained_leaf(source_page, complete, seed, owner)
            members = [root]
        else:
            middle = len(complete) // 2
            assert 0 < middle < len(complete)
            assert frames.framing(complete[:middle], 0)[1] <= CAPACITY
            assert frames.framing(complete[middle:], 0)[1] <= CAPACITY
            right = len(raw) // PAGE
            left = right + 1
            raw.extend(bytes(2 * PAGE))
            raw[right * PAGE:(right + 1) * PAGE] = frames.leaf(None, complete[middle:], 0, owner, left, 0)
            raw[left * PAGE:(left + 1) * PAGE] = frames.leaf(None, complete[:middle], 0, owner, 0, right)
            after_page = grown_root(source_page, complete[middle - 1], left, right, owner)
            members = [root, left, right]
            for number in (right, left):
                allocations.append({'physical': physical, 'page': number, 'purpose': 'INDEX'})
                for selected, set_bit in ((mapping, True), (facts['maps']['GlobalFree'], False)):
                    offset, bit = map_cell(original_raw, selected, number)
                    assert bool(original_raw[offset] & bit) != set_bit
                    raw[offset] = raw[offset] | bit if set_bit else raw[offset] & ~bit
                    map_offsets.add(offset)
        raw[root * PAGE:(root + 1) * PAGE] = after_page
        allowed.update(range(root * PAGE, (root + 1) * PAGE))
        delta = history(original, final, index['columns'], ignored)
        counter_at = owner * PAGE + index['history_offset']
        assert int.from_bytes(original_raw[counter_at:counter_at + 4], 'little') == index['history']
        next_count = index['history'] + delta
        assert 0 <= next_count < 1 << 32
        raw[counter_at:counter_at + 4] = next_count.to_bytes(4, 'little')
        allowed.update(range(counter_at, counter_at + 4))
        physical_details.append({'physical': physical, 'root': root, 'columns': index['columns'],
                                 'entries': len(complete), 'entry_hex': [entry.hex() for entry in complete],
                                 'members': sorted(members), 'source_prefix': seed,
                                 'fitting_prefix': prefix, 'fitting_active': active,
                                 'history_before': index['history'], 'history_delta': delta,
                                 'history_after': next_count})
    allowed.update(map_offsets)
    differences = [offset for offset, (before_byte, after_byte) in enumerate(zip(original_raw, raw))
                   if before_byte != after_byte]
    assert all(offset in allowed for offset in differences)
    assert len(raw) == len(original_raw) + len(allocations) * PAGE
    assert len(changed_roots) == (2 if case['recipe'] == 'DuplicateFamily' else 500)
    assert sum(item['expected'] != item['value'] for item in assignments) == (4 if case['recipe'] == 'DuplicateFamily' else 800)
    assert len(allocations) == (0 if case['recipe'] == 'DuplicateFamily' else 4)
    expected_delta = 1 if case['recipe'] == 'DuplicateFamily' else 300
    assert [item['history_delta'] for item in physical_details] == [0, expected_delta]
    return bytes(raw), {'case': case['id'], 'table': case['table'], 'recipe': case['recipe'],
                        'assignments': assignments, 'changed_roots': sorted(changed_roots),
                        'changed_cells': sum(item['expected'] != item['value'] for item in assignments),
                        'allocations': allocations, 'physical': physical_details,
                        'changed_source_offsets': differences, 'map_offsets': sorted(map_offsets),
                        'source_pages': len(original_raw) // PAGE, 'final_pages': len(raw) // PAGE,
                        'authority': 'Qualified immutable SOURCE plus frozen requests; no AFTER or producer inputs.'}
