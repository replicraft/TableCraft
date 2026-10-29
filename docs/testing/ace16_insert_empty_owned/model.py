"""SOURCE/request-only first DATA and ordinary-owned block-zero construction."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_multiple_indexes import model as frames
from ace16_insert_global_bootstrap import source as wire

PAGE = 4096
HEADER_BYTES = 14
SLOT_BYTES = 2
MAX_ROWS = 255
POINTER_BYTES = 4
REFERENCE_BYTES = 69


def record(identity, payload):
    assert isinstance(identity, int) and not isinstance(identity, bool)
    assert -(2 ** 31) <= identity < 2 ** 31
    assert isinstance(payload, bytes) and 0 < len(payload) <= 255
    result = bytearray(b'\x02\0')
    result.extend(identity.to_bytes(4, 'little', signed=True))
    result.extend(payload)
    result.extend(len(result).to_bytes(2, 'little'))
    result.extend(b'\x06\0\x01\0\x03')
    return bytes(result)


def derive(state, requests):
    original = state['raw']
    table = state['table']
    definition = table['definition']
    owner = 'Table:' + str(definition)
    assert table['count'] == 0 and table['rows'] == {} and table['values'] == {}
    assert list(table['columns']) == ['ID', 'Bytes']
    assert state['maps'][owner + ':Owned']['file_members'] == []
    assert state['maps'][owner + ':Hint']['file_members'] == []
    assert state['trees'][0]['entries'] == []
    assert len({identity for identity, _ in requests}) == len(requests)
    values = [(identity, bytes.fromhex(payload)) for identity, payload in requests]
    bodies = [record(identity, payload) for identity, payload in values]
    assert all(len(body) + SLOT_BYTES <= PAGE - HEADER_BYTES for body in bodies)
    raw = bytearray(original)
    allocations = []
    data_pages = []
    hints = set()
    bitmap = None
    placements = []
    allowed = bytearray(len(original))

    def allow(begin, end):
        assert 0 <= begin <= end <= len(original)
        allowed[begin:end] = bytes([255]) * (end - begin)

    def map_bit(role, page, present):
        claim = state['map_claims'][role]
        offset, mask = wire.map_cell(original, claim, page)
        before = bool(raw[offset] & mask)
        assert before != present, (role, page, present)
        raw[offset] = raw[offset] | mask if present else raw[offset] & ~mask
        allowed[offset] |= mask

    def reserve(purpose):
        number = len(raw) // PAGE
        assert number in state['certified_future_pages']
        assert number >= len(original) // PAGE
        for role, claim in state['map_claims'].items():
            cell = wire.represented_cell(original, claim, number)
            if role == 'GlobalFree':
                assert cell is not None and original[cell[0]] & cell[1]
            elif cell is not None:
                assert not original[cell[0]] & cell[1], (role, number)
        raw.extend(bytes(PAGE))
        map_bit('GlobalFree', number, False)
        allocations.append({'page': number, 'purpose': purpose})
        return number

    def add_owned(number):
        assert bitmap is not None and number != bitmap
        offset = bitmap * PAGE + 4 + number // 8
        mask = 1 << (number % 8)
        assert not raw[offset] & mask
        raw[offset] |= mask

    for (identity, payload), body in zip(values, bodies, strict=True):
        highest = max(hints) if hints else None
        if highest is not None:
            free = wire.number(raw, highest * PAGE + 2)
            count = wire.number(raw, highest * PAGE + 12)
            if count >= MAX_ROWS or free < len(body) + SLOT_BYTES:
                map_bit(owner + ':Hint', highest, False)
                hints.remove(highest)
                highest = None
        if highest is None:
            highest = reserve('DATA')
            data_pages.append(highest)
            base = highest * PAGE
            raw[base:base + 2] = b'\x01\x01'
            frames.put(raw, base + 2, PAGE - HEADER_BYTES, 2)
            frames.put(raw, base + 4, definition)
            if bitmap is None:
                claim = state['map_claims'][owner + ':Owned']
                begin, end = claim['row_start'], claim['row_end']
                before = original[begin:end]
                assert len(before) == REFERENCE_BYTES and before[0] == 1
                assert wire.number(before, 1, POINTER_BYTES) == 0
                bitmap = reserve('ORDINARY_OWNED_BITMAP')
                raw[bitmap * PAGE:bitmap * PAGE + 4] = b'\x05\x01\0\0'
                replacement = bytearray(before)
                replacement[1:5] = bitmap.to_bytes(POINTER_BYTES, 'little')
                raw[begin:end] = replacement
                allow(begin, end)
                assert raw[begin + 5:end] == original[begin + 5:end]
            add_owned(highest)
            map_bit(owner + ':Hint', highest, True)
            hints.add(highest)
        base = highest * PAGE
        count = wire.number(raw, base + 12)
        end = PAGE if count == 0 else wire.number(raw, base + 12 + count * SLOT_BYTES)
        begin = end - len(body)
        assert begin >= HEADER_BYTES + (count + 1) * SLOT_BYTES
        raw[base + begin:base + end] = body
        frames.put(raw, base + HEADER_BYTES + count * SLOT_BYTES, begin, SLOT_BYTES)
        frames.put(raw, base + 12, count + 1, 2)
        frames.put(raw, base + 2, begin - HEADER_BYTES - (count + 1) * SLOT_BYTES, 2)
        placements.append({'identity': identity, 'page': highest, 'slot': count,
                           'reference': highest << 8 | count, 'record_bytes': len(body)})

    index = table['indexes']['PrimaryID']
    root = index['root']
    entries = []
    for (identity, _), placed in zip(values, placements, strict=True):
        key = b'\x7f' + ((identity & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
        entries.append(key + placed['reference'].to_bytes(4, 'big'))
    entries.sort()
    before_root = original[root * PAGE:(root + 1) * PAGE]
    seed = wire.number(before_root, 24)
    assert frames.framing(entries, seed)[1] <= 3616
    raw[root * PAGE:(root + 1) * PAGE] = frames.leaf(before_root, entries, seed, definition)
    allow(root * PAGE, root * PAGE + 480 + frames.framing(entries, seed)[1])
    frames.put(raw, definition * PAGE + 16, len(values))
    history_at = state['trees'][0]['history_at']
    assert wire.number(original, history_at, 4) == 0
    frames.put(raw, history_at, len({identity for identity, _ in values}))
    allow(definition * PAGE + 16, definition * PAGE + 20)
    allow(history_at, history_at + 4)

    for index, (before, after) in enumerate(zip(original, raw)):
        assert (before ^ after) & ~allowed[index] == 0, ('Protected source byte', index)
    decoded = {}
    for role, source_map in state['maps'].items():
        if role == owner + ':Owned':
            members, total = data_pages, len(data_pages)
        elif role == owner + ':Hint':
            members, total = sorted(hints), len(hints)
        elif role == 'GlobalFree':
            members = source_map['file_members']
            total = source_map['complete_set_bit_count'] - len(allocations)
        else:
            members, total = source_map['file_members'], source_map['complete_set_bit_count']
        description = {
            'table_page': source_map['map_page'],
            'row_start': source_map['row_start'] - source_map['map_page'] * PAGE,
            'row_end': source_map['row_end'] - source_map['map_page'] * PAGE,
            'file_members': list(members),
            'reported_count': total,
        }
        decoded[role] = wire.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    assert decoded[owner + ':Owned']['bitmap_pages'] == [bitmap] + state['maps'][owner + ':Owned']['bitmap_pages']
    assert bitmap not in decoded[owner + ':Owned']['file_members']
    assert bitmap not in decoded[owner + ':Hint']['file_members']
    nodes, leaves, actual_entries = wire.index_wire.tree(raw, root, definition)
    assert list(nodes) == leaves == [root] and actual_entries == entries
    for (identity, payload), placed in zip(values, placements, strict=True):
        base = placed['page'] * PAGE
        count = wire.number(raw, base + 12)
        slot = placed['slot']
        begin = wire.number(raw, base + HEADER_BYTES + slot * SLOT_BYTES)
        end = PAGE if slot == 0 else wire.number(raw, base + 12 + slot * SLOT_BYTES)
        body = raw[base + begin:base + end]
        mask = len(body) - 1
        assert wire.number(body, 0) == 2 and body[mask] == 3
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        assert wire.number(body, mask - 2) == 1
        assert wire.number(body, mask - 4) == 6
        variable_end = wire.number(body, mask - 6)
        assert variable_end == mask - 6 and body[6:variable_end] == payload
        assert slot < count <= MAX_ROWS
        last = wire.number(raw, base + 12 + count * SLOT_BYTES)
        assert wire.number(raw, base + 2) == last - HEADER_BYTES - count * SLOT_BYTES
    result = bytes(raw)
    return result, {
        'sha256': hashlib.sha256(result).hexdigest(), 'bytes': len(result),
        'source_pages': len(original) // PAGE, 'final_pages': len(result) // PAGE,
        'definition': definition, 'root': root, 'allocations': allocations,
        'data_pages': data_pages, 'owned_bitmap': bitmap, 'hints': sorted(hints),
        'rows': placements, 'table_count': len(values), 'physical_history': len(values),
        'all_higher_owned_pointers_preserved': True,
        'all_unrelated_source_bytes_preserved': True,
    }
