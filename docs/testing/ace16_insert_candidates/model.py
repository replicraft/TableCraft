"""Independent two-field INSERT images from immutable sources and explicit values."""
import hashlib

PAGE = 4096
DATA_TAG = bytes([1, 1])
INDEX_LEAF_TAG = bytes([4, 1])
BITMAP_TAG = bytes([5, 1, 0, 0])
MAP_BYTES = 69
INLINE_BITS = 512
BITMAP_PREFIX = 4
BITMAP_BITS = (PAGE - BITMAP_PREFIX) * 8
DIRECTORY = 14
SLOT_BYTES = 2
SLOT_MASK = 0x1fff
FREE = 2
OWNER = 4
SLOTS = 12
ENTRY_START = 480
ENTRY_BITMAP = 27
PREFIX = 24
CAPACITY = PAGE - ENTRY_START
TABLE_COUNT = 16
MAX_COLUMNS = 41
VARIABLE_SLOTS = 43
COLUMN_COUNT = 45
PHYSICAL_COUNT = 51
COLUMN_BLOCK = 63
COUNTER_BYTES = 12
HISTORICAL_IN_BLOCK = 4
COLUMN_BYTES = 25
PHYSICAL_BYTES = 52
USAGE_IN_PHYSICAL = 34
ROOT_IN_PHYSICAL = 38
FLAGS_IN_PHYSICAL = 46


def number(raw, offset, width=4):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


def write(raw, offset, value, width):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def frame(raw, page, slot, expected_flags=0):
    block = raw[page * PAGE:(page + 1) * PAGE]
    assert len(block) == PAGE and block[:2] == DATA_TAG
    count = number(block, SLOTS, 2)
    assert 0 <= slot < count <= 255
    word = number(block, DIRECTORY + slot * SLOT_BYTES, 2)
    assert word & ~SLOT_MASK == expected_flags
    start = word & SLOT_MASK
    end = PAGE if slot == 0 else number(block, DIRECTORY + (slot - 1) * SLOT_BYTES, 2) & SLOT_MASK
    assert DIRECTORY + count * SLOT_BYTES <= start < end <= PAGE
    return page * PAGE + start, page * PAGE + end


def map_record(raw, reference):
    page, slot = reference >> 8, reference & 255
    start, end = frame(raw, page, slot)
    assert end - start == MAP_BYTES
    row = raw[start:end]
    assert row[0] in (0, 1)
    members = set()
    locations = {}
    if row[0] == 0:
        base = number(row, 1)
        regions = [(base, start + 5, row[5:])]
    else:
        pointers = [number(row, 1 + position * 4) for position in range(16)]
        assert pointers[0] and not any(pointers[1:])
        bitmap = pointers[0]
        assert bitmap < len(raw) // PAGE
        block = raw[bitmap * PAGE:(bitmap + 1) * PAGE]
        assert block[:4] == BITMAP_TAG
        regions = [(0, bitmap * PAGE + BITMAP_PREFIX, block[BITMAP_PREFIX:])]
    for base, absolute, payload in regions:
        for byte_position, value in enumerate(payload):
            for bit in range(8):
                member = base + byte_position * 8 + bit
                locations[member] = (absolute + byte_position, 1 << bit)
                if value & (1 << bit):
                    members.add(member)
    return {
        'reference': reference,
        'range': (start, end),
        'kind': row[0],
        'members': members,
        'locations': locations,
    }


def schema(raw, table):
    assert table['columns_count'] == table['max_columns'] == 2
    assert list(table['columns']) == ['ID', 'Bytes']
    assert table['properties'] == []
    assert len(table['indexes']) == 1
    index = table['indexes']['PrimaryID']
    assert index['physical'] == 0
    assert index['primary'] and index['unique'] and index['required']
    assert not index['ignored_null'] and index['owned_pages'] == 1
    assert index['columns'] == [['ID', 'true']]
    definition = table['definition']
    base = definition * PAGE
    assert raw[base:base + 2] == bytes([2, 1])
    assert number(raw, base + PHYSICAL_COUNT) == 1
    assert number(raw, base + 47) == 1
    assert number(raw, base + MAX_COLUMNS, 2) == number(raw, base + COLUMN_COUNT, 2) == 2
    assert number(raw, base + VARIABLE_SLOTS, 2) == 1
    fields = base + COLUMN_BLOCK + COUNTER_BYTES
    expected = [(4, 0, 4, 3, 0), (9, 1, 255, 2, 0)]
    for position, (kind, native_number, width, flags, offset) in enumerate(expected):
        column = raw[fields + position * COLUMN_BYTES:fields + (position + 1) * COLUMN_BYTES]
        assert len(column) == COLUMN_BYTES
        assert column[0] == kind and number(column, 5, 2) == native_number
        assert (column[15], column[16]) == (flags, 0)
        if kind == 4:
            assert number(column, 21, 2) == offset
        assert number(column, 23, 2) == width
    position = fields + 2 * COLUMN_BYTES
    for expected_name in ('ID', 'Bytes'):
        length = number(raw, position, 2)
        position += 2
        assert raw[position:position + length].decode('utf-16le') == expected_name
        position += length
    physical = raw[position:position + PHYSICAL_BYTES]
    assert len(physical) == PHYSICAL_BYTES
    assert number(physical, ROOT_IN_PHYSICAL) == index['root']
    assert physical[FLAGS_IN_PHYSICAL] == 0x89
    assert number(physical, 4, 2) == 0 and physical[6] == 1
    for key in range(1, 10):
        assert number(physical, 4 + key * 3, 2) == 0xffff
    return {
        'definition': definition,
        'table_count': base + TABLE_COUNT,
        'history': base + COLUMN_BLOCK + HISTORICAL_IN_BLOCK,
        'root': index['root'],
        'owned': number(raw, base + 55),
        'hint': number(raw, base + 59),
        'index_owned': number(physical, USAGE_IN_PHYSICAL),
    }


def int_key(identity, reference):
    assert -(1 << 31) <= identity < 1 << 31
    assert 0 <= reference < 1 << 32
    signed = (identity & 0xffffffff) ^ 0x80000000
    return bytes([0x7f]) + signed.to_bytes(4, 'big') + reference.to_bytes(4, 'big')


def primary_entries(raw, root, owner):
    page = raw[root * PAGE:(root + 1) * PAGE]
    assert page[:2] == INDEX_LEAF_TAG
    assert number(page, OWNER) == owner
    assert page[26] == 0
    assert number(page, 12) == number(page, 16) == number(page, 20) == 0
    prefix = number(page, PREFIX, 2)
    ends = [
        position * 8 + bit
        for position, value in enumerate(page[ENTRY_BITMAP:ENTRY_START])
        for bit in range(8)
        if value & (1 << bit)
    ]
    entries = []
    start = 0
    for end in ends:
        assert start < end <= CAPACITY
        suffix = page[ENTRY_START + start:ENTRY_START + end]
        entry = suffix if not entries else entries[0][:prefix] + suffix
        assert len(entry) == 9 and entry[0] == 0x7f
        if entries:
            assert entry > entries[-1]
        entries.append(entry)
        start = end
    assert entries and number(page, FREE, 2) == CAPACITY - start
    return entries


def common(entries):
    shared = min(map(len, entries))
    for entry in entries[1:]:
        shared = min(shared, next((i for i in range(shared) if entry[i] != entries[0][i]), shared))
    return shared


def retained_leaf(original, entries):
    assert entries
    maximum = common(entries)
    prefix = min(number(original, PREFIX, 2), maximum)
    length = sum(map(len, entries)) - prefix * (len(entries) - 1)
    if length > CAPACITY:
        prefix = maximum
        length = sum(map(len, entries)) - prefix * (len(entries) - 1)
    assert length <= CAPACITY
    result = bytearray(original)
    write(result, FREE, CAPACITY - length, 2)
    write(result, PREFIX, prefix, 2)
    result[ENTRY_BITMAP:ENTRY_START] = bytes(ENTRY_START - ENTRY_BITMAP)
    position = 0
    for ordinal, entry in enumerate(entries):
        suffix = entry if ordinal == 0 else entry[prefix:]
        result[ENTRY_START + position:ENTRY_START + position + len(suffix)] = suffix
        position += len(suffix)
        result[ENTRY_BITMAP + position // 8] |= 1 << (position % 8)
    assert position == length
    assert result[ENTRY_START + length:] == original[ENTRY_START + length:]
    return bytes(result), length, prefix


def record(identity, payload):
    assert isinstance(identity, int) and not isinstance(identity, bool)
    assert -(1 << 31) <= identity < 1 << 31
    assert isinstance(payload, bytes) and len(payload) == 255
    return (
        bytes([2, 0]) + identity.to_bytes(4, 'little', signed=True) + payload
        + bytes.fromhex('05010600010003')
    )


def derive(raw, table, identity, payload):
    metadata = schema(raw, table)
    body = record(identity, payload)
    assert len(body) == 268
    assert identity not in table['rows']
    before_count = number(raw, metadata['table_count'])
    before_history = number(raw, metadata['history'])
    assert before_count == len(table['rows']) == table['count']
    assert before_count < 0xffffffff and before_history < 0xffffffff
    owned = map_record(raw, metadata['owned'])
    hint = map_record(raw, metadata['hint'])
    global_free = map_record(raw, 1 << 8)
    assert hint['members'] <= owned['members']
    assert hint['members']
    destination = max(hint['members'])
    assert destination < len(raw) // PAGE
    page = raw[destination * PAGE:(destination + 1) * PAGE]
    assert page[:2] == DATA_TAG and number(page, OWNER) == metadata['definition']
    count = number(page, SLOTS, 2)
    assert 0 < count <= 255
    lowest = min(number(page, DIRECTORY + slot * 2, 2) & SLOT_MASK for slot in range(count))
    assert lowest >= DIRECTORY + count * SLOT_BYTES
    free = lowest - DIRECTORY - count * SLOT_BYTES
    assert number(page, FREE, 2) == free
    fitted = count < 255 and len(body) + SLOT_BYTES <= free
    result = bytearray(raw)
    effects = {}

    def bit(usage, member, value):
        offset, mask = usage['locations'][member]
        before = raw[offset]
        current = effects.setdefault(offset, before)
        assert bool(current & mask) != value
        effects[offset] = current | mask if value else current & ~mask

    if fitted:
        slot = count
        target = destination
        start = lowest - len(body)
        changed = bytearray(page)
        write(changed, FREE, free - len(body) - SLOT_BYTES, 2)
        write(changed, SLOTS, count + 1, 2)
        write(changed, DIRECTORY + slot * SLOT_BYTES, start, 2)
        changed[start:lowest] = body
        result[target * PAGE:(target + 1) * PAGE] = changed
        after_free = free - len(body) - SLOT_BYTES
    else:
        assert count < 255 and free < len(body) + SLOT_BYTES
        bit(hint, destination, False)
        target = len(raw) // PAGE
        slot = 0
        start = PAGE - len(body)
        changed = bytearray(PAGE)
        changed[:2] = DATA_TAG
        after_free = PAGE - DIRECTORY - SLOT_BYTES - len(body)
        write(changed, FREE, after_free, 2)
        write(changed, OWNER, metadata['definition'], 4)
        write(changed, SLOTS, 1, 2)
        write(changed, DIRECTORY, start, 2)
        changed[start:] = body
        result.extend(changed)
        bit(global_free, target, False)
        bit(owned, target, True)
        bit(hint, target, True)
    for offset, value in effects.items():
        result[offset] = value
    reference = target << 8 | slot
    original_entries = primary_entries(raw, metadata['root'], metadata['definition'])
    expected_entries = [
        int_key(old_id, row['page'] << 8 | row['slot'])
        for old_id, row in table['rows'].items()
    ]
    assert sorted(expected_entries) == original_entries
    new_entry = int_key(identity, reference)
    entries = sorted(original_entries + [new_entry])
    assert len(set(entry[:5] for entry in entries)) == len(entries)
    root_start = metadata['root'] * PAGE
    rewritten, active, prefix = retained_leaf(raw[root_start:root_start + PAGE], entries)
    result[root_start:root_start + PAGE] = rewritten
    write(result, metadata['table_count'], before_count + 1, 4)
    write(result, metadata['history'], before_history + 1, 4)
    final = bytes(result)
    assert primary_entries(final, metadata['root'], metadata['definition']) == entries
    assert final[target * PAGE + start:target * PAGE + start + len(body)] == body
    for old_id, row in table['rows'].items():
        first, end = frame(raw, row['page'], row['slot'])
        now_first, now_end = frame(final, row['page'], row['slot'])
        assert (first, end) == (now_first, now_end)
        assert raw[first:end] == final[now_first:now_end]
    return final, {
        'definition': metadata['definition'],
        'primary_root': metadata['root'],
        'primary_prefix': prefix,
        'primary_active_bytes': active,
        'source_count': before_count,
        'final_count': before_count + 1,
        'source_historical_count': before_history,
        'final_historical_count': before_history + 1,
        'table_count_offset': metadata['table_count'],
        'historical_count_offset': metadata['history'],
        'placement': 'fitted-visible-root' if fitted else 'EOF-visible-root',
        'page': target,
        'slot': slot,
        'reference': reference,
        'root_flags': 0,
        'record_bytes': len(body),
        'record_sha256': digest(body),
        'free_before': free,
        'free_after': after_free,
        'map_byte_effects': [{'offset': offset, 'before': raw[offset], 'after': value} for offset, value in sorted(effects.items())],
        'bytes_added': len(final) - len(raw),
        'expected_image_bytes': len(final),
        'expected_image_sha256': digest(final),
        'old_rows_and_bodies_preserved': len(table['rows']),
        'primary_entry_count': len(entries),
    }
