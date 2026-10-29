"""Independent raw SOURCE/companion-request increment images, with no AFTER input."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_multiple_indexes import model as frames
from ace16_insert_global_bootstrap import source as wire

PAGE = 4096
CAPACITY = 3616
MAX_SLOTS = 255


def record(earlier, identity, score):
    assert all(isinstance(value, int) and not isinstance(value, bool)
               and -(1 << 31) <= value < 1 << 31 for value in (earlier, identity))
    assert score is None or (isinstance(score, int) and not isinstance(score, bool)
                             and -(1 << 31) <= score < 1 << 31)
    return (b'\x03\x00' + earlier.to_bytes(4, 'little', signed=True)
            + identity.to_bytes(4, 'little', signed=True)
            + (0 if score is None else score).to_bytes(4, 'little', signed=True)
            + bytes([3 if score is None else 7]))


def key(value, reference):
    payload = b'\x00' if value is None else b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return payload + reference.to_bytes(4, 'big')


def derive(state, case):
    original = state['raw']
    table = state['tables']['Items']
    owner = table['definition']
    prefix = 'Table:' + str(owner)
    assert case['expected_last'] == state['counter_last']
    requests = case['requests']
    assert 0 < len(requests) <= 4096
    generated = [state['counter_last'] + position + 1 for position in range(len(requests))]
    assert all(-(1 << 31) <= identity < 1 << 31 for identity in generated)
    assert not set(generated) & set(table['rows'])
    assert len(generated) == len(set(generated))
    raw = bytearray(original)
    allowed = bytearray(len(original))
    maps = state['maps']
    claims = state['claims']
    hints = set(maps[prefix + ':Hint']['file_members'])
    owned = set(maps[prefix + ':Owned']['file_members'])
    expected_members = {role: set(item['file_members']) for role, item in maps.items()}
    allocations = []
    placements = []
    all_rows = {reference: list(values) for reference, values in state['live_rows'].items()}
    owned_bitmap = None
    original_owned = claims[prefix + ':Owned']
    if original[original_owned['row_start']] == 1:
        owned_bitmap = wire.number(original, original_owned['row_start'] + 1, 4) or None

    def permit(begin, end):
        assert 0 <= begin <= end <= len(original)
        allowed[begin:end] = bytes([255]) * (end - begin)

    def map_bit(role, page, present):
        at, mask = wire.map_cell(raw, claims[role], page)
        before = bool(raw[at] & mask)
        assert before != present, (role, page, present)
        raw[at] = raw[at] | mask if present else raw[at] & ~mask
        if at < len(original):
            allowed[at] |= mask
        if present:
            expected_members[role].add(page)
        else:
            expected_members[role].discard(page)

    def reserve(purpose, physical=None):
        number = len(raw) // PAGE
        assert number < len(original) // PAGE + 8
        for role, claim in claims.items():
            cell = wire.represented_cell(original, claim, number)
            if role == 'GlobalFree':
                assert cell is not None and original[cell[0]] & cell[1]
            elif cell is not None:
                assert not original[cell[0]] & cell[1], (role, number)
        raw.extend(bytes(PAGE))
        map_bit('GlobalFree', number, False)
        allocations.append(dict(page=number, purpose=purpose, physical=physical))
        return number

    def add_data():
        nonlocal owned_bitmap
        number = reserve('DATA')
        start = number * PAGE
        raw[start:start + 2] = b'\x01\x01'
        frames.put(raw, start + 2, PAGE - 14, 2)
        frames.put(raw, start + 4, owner)
        claim = claims[prefix + ':Owned']
        begin, end = claim['row_start'], claim['row_end']
        assert raw[begin] == 1 and end - begin == 69
        if owned_bitmap is None:
            assert not owned and wire.number(original, begin + 1, 4) == 0
            owned_bitmap = reserve('ORDINARY_OWNED_BITMAP')
            raw[owned_bitmap * PAGE:owned_bitmap * PAGE + 4] = b'\x05\x01\x00\x00'
            frames.put(raw, begin + 1, owned_bitmap)
            permit(begin + 1, begin + 5)
            assert raw[begin + 5:end] == original[begin + 5:end]
        map_bit(prefix + ':Owned', number, True)
        map_bit(prefix + ':Hint', number, True)
        owned.add(number)
        hints.add(number)
        return number

    for position, ((earlier, score), identity) in enumerate(zip(requests, generated, strict=True)):
        body = record(earlier, identity, score)
        page = max(hints) if hints else None
        if page is not None:
            at = page * PAGE
            count = wire.number(raw, at + 12)
            free = wire.number(raw, at + 2)
            if count >= MAX_SLOTS or free < len(body) + 2:
                map_bit(prefix + ':Hint', page, False)
                hints.remove(page)
                page = None
        if page is None:
            page = add_data()
        base = page * PAGE
        slot = wire.number(raw, base + 12)
        assert slot < MAX_SLOTS
        end = PAGE if slot == 0 else wire.number(raw, base + 12 + slot * 2) & 0x1fff
        begin = end - len(body)
        assert begin >= 14 + (slot + 1) * 2
        raw[base + begin:base + end] = body
        frames.put(raw, base + 14 + slot * 2, begin, 2)
        frames.put(raw, base + 12, slot + 1, 2)
        frames.put(raw, base + 2, begin - 14 - (slot + 1) * 2, 2)
        if base < len(original):
            permit(base + begin, base + end)
            permit(base + 14 + slot * 2, base + 16 + slot * 2)
            permit(base + 12, base + 14)
            permit(base + 2, base + 4)
        reference = page << 8 | slot
        assert reference not in all_rows
        all_rows[reference] = [earlier, identity, score]
        placements.append(dict(position=position, identity=identity, reference=reference,
                               page=page, slot=slot, raw=body.hex()))

    histories = []
    index_members = {}
    for physical, name, column in [(0, 'PrimaryID', 1), (1, 'ScoreOrder', 2)]:
        index = table['indexes'][name]
        assert index['physical'] == physical
        root = index['root']
        page_before = original[root * PAGE:(root + 1) * PAGE]
        assert page_before[:2] == b'\x04\x01'
        entries = sorted(key(values[column], reference) for reference, values in all_rows.items())
        seed = wire.number(page_before, 24)
        if frames.framing(entries, seed)[1] <= CAPACITY:
            replacement = frames.leaf(page_before, entries, seed, owner)
            members = {root}
        else:
            middle = len(entries) // 2
            right = reserve('INDEX', physical)
            left = reserve('INDEX', physical)
            raw[right * PAGE:(right + 1) * PAGE] = frames.leaf(None, entries[middle:], 0, owner, left, 0)
            raw[left * PAGE:(left + 1) * PAGE] = frames.leaf(None, entries[:middle], 0, owner, 0, right)
            replacement = frames.root(page_before, entries[middle - 1], left, right, owner)
            members = {root, left, right}
        raw[root * PAGE:(root + 1) * PAGE] = replacement
        active = CAPACITY - wire.number(replacement, 2)
        permit(root * PAGE, root * PAGE + 480 + active)
        source_members = set(maps[prefix + ':IndexData:' + str(physical)]['file_members'])
        assert source_members == {root}
        for added in sorted(members - source_members):
            map_bit(prefix + ':IndexData:' + str(physical), added, True)
        index_members[physical] = members
        original_families = {values[column] for values in state['live_rows'].values()}
        final_families = {values[column] for values in all_rows.values()}
        delta = len(final_families - original_families)
        history_at = owner * PAGE + index['history_offset']
        assert wire.number(original, history_at, 4) == index['history']
        after = index['history'] + delta
        assert after <= 0xffffffff
        frames.put(raw, history_at, after)
        permit(history_at, history_at + 4)
        histories.append(dict(physical=physical, before=index['history'], delta=delta, after=after))

    frames.put(raw, owner * PAGE + 16, len(all_rows))
    raw[owner * PAGE + 20:owner * PAGE + 24] = generated[-1].to_bytes(4, 'little', signed=True)
    permit(owner * PAGE + 16, owner * PAGE + 24)
    assert raw[owner * PAGE + 24:owner * PAGE + 32] == original[owner * PAGE + 24:owner * PAGE + 32]
    for at, (before, after) in enumerate(zip(original, raw)):
        assert (before ^ after) & ~allowed[at] == 0, ('Protected source byte', at)
    # All original DATA bodies and directory cells, including tombstones, remain
    # byte-exact. Only newly appended slots have new records.
    for page in maps[prefix + ':Owned']['file_members']:
        base = page * PAGE
        count = wire.number(original, base + 12)
        assert raw[base + 14:base + 14 + 2 * count] == original[base + 14:base + 14 + 2 * count]
        end = PAGE
        for slot in range(count):
            begin = wire.number(original, base + 14 + slot * 2) & 0x1fff
            assert raw[base + begin:base + end] == original[base + begin:base + end]
            end = begin
    for placement, (earlier, score) in zip(placements, requests, strict=True):
        frame, _ = wire.long_wire.final_frame(raw, placement)
        body = raw[frame['start']:frame['end']]
        assert body == record(earlier, placement['identity'], score)
        assert [int.from_bytes(body[2:6], 'little', signed=True),
                int.from_bytes(body[6:10], 'little', signed=True),
                None if body[-1] & 4 == 0 else int.from_bytes(body[10:14], 'little', signed=True)] == [earlier, placement['identity'], score]
    for physical, name, column in [(0, 'PrimaryID', 1), (1, 'ScoreOrder', 2)]:
        nodes, leaves, actual = wire.index_wire.tree(bytes(raw), table['indexes'][name]['root'], owner)
        assert set(nodes) == index_members[physical]
        assert actual == sorted(key(values[column], reference) for reference, values in all_rows.items())
        assert len(leaves) == (2 if len(nodes) == 3 else 1)
    for role, before in maps.items():
        wanted = expected_members[role]
        total = before['complete_set_bit_count'] + len(wanted - set(before['file_members'])) - len(set(before['file_members']) - wanted)
        if role == 'GlobalFree':
            total = before['complete_set_bit_count'] - len(allocations)
        description = dict(table_page=before['map_page'], row_start=before['row_start'] - before['map_page'] * PAGE,
                           row_end=before['row_end'] - before['map_page'] * PAGE,
                           file_members=sorted(wanted), reported_count=total)
        after = wire.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
        if role == prefix + ':Owned' and not maps[role]['bitmap_pages']:
            assert after['bitmap_pages'] == [owned_bitmap]
            assert owned_bitmap not in after['file_members']
    assert len(allocations) == len(raw) // PAGE - len(original) // PAGE
    result = bytes(raw)
    return result, dict(sha256=hashlib.sha256(result).hexdigest(), bytes=len(result),
                        source_pages=len(original) // PAGE, final_pages=len(raw) // PAGE,
                        definition=owner, counter_before=state['counter_last'], counter_after=generated[-1],
                        generated_values=generated, generated_column_number=1,
                        placements=placements, allocations=allocations,
                        histories=histories, rows=len(all_rows),
                        original_live_and_deleted_bodies_preserved=True,
                        mode='explicit caller Increment engineering policy; source stored mode unclassified')
