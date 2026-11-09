"""SOURCE+REQUEST engineering image; no public AFTER image supplies bytes."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_multiple_indexes import reference

PAGE = 4096
CAPACITY = 3616


def put(raw, at, value, width=4):
    raw[at:at + width] = value.to_bytes(width, 'little')


def framing(entries, seed):
    if not entries:
        return 0, 0
    common = min(map(len, entries))
    for entry in entries[1:]:
        count = 0
        while count < common and entries[0][count] == entry[count]:
            count += 1
        common = count
    full = sum(map(len, entries))
    prefix = min(seed, common)
    if full - prefix * (len(entries) - 1) > CAPACITY:
        prefix = common
    return prefix, full - prefix * (len(entries) - 1)


def leaf(original, entries, seed, owner, previous=0, next_=0):
    prefix, active = framing(entries, seed)
    assert active <= CAPACITY
    page = bytearray(original) if original is not None else bytearray(PAGE)
    page[:480] = bytes(480)
    page[:2] = bytes([4, 1])
    put(page, 2, CAPACITY - active, 2)
    put(page, 4, owner)
    put(page, 12, previous)
    put(page, 16, next_)
    put(page, 24, prefix, 2)
    end = 0
    for ordinal, entry in enumerate(entries):
        suffix = entry[0 if ordinal == 0 else prefix:]
        page[480 + end:480 + end + len(suffix)] = suffix
        end += len(suffix)
        page[27 + end // 8] |= 1 << (end % 8)
    assert end == active
    return page


def root(original, bound, left, right, owner):
    page = bytearray(original)
    page[:480] = bytes(480)
    page[:2] = bytes([3, 1])
    active = len(bound) + 4
    put(page, 2, CAPACITY - active, 2)
    put(page, 4, owner)
    put(page, 20, right)
    page[26] = 1
    page[480:480 + len(bound)] = bound
    page[480 + len(bound):480 + active] = left.to_bytes(4, 'big')
    page[27 + active // 8] |= 1 << (active % 8)
    return page


def derive(case, before):
    raw = bytearray(before['raw'])
    definition = before['table']['definition']
    all_rows = dict(before['rows'])
    hints = set(before['maps']['hint']['members'])
    new_data = []
    placements = []
    for requested in reference.requests(case['variant']):
        body = reference.record(requested)
        highest = max(hints)
        free = reference.n(raw, highest * PAGE + 2, 2)
        if free >= len(body) + 2:
            page = highest
        else:
            hints.remove(highest)
            page = len(raw) // PAGE
            raw.extend(bytes(PAGE))
            raw[page * PAGE:page * PAGE + 2] = bytes([1, 1])
            put(raw, page * PAGE + 4, definition)
            put(raw, page * PAGE + 2, PAGE - 14, 2)
            hints.add(page)
            new_data.append(page)
        base = page * PAGE
        slot = reference.n(raw, base + 12, 2)
        assert slot < 255
        end = PAGE if slot == 0 else reference.n(raw, base + 12 + 2 * slot, 2)
        begin = end - len(body)
        raw[base + begin:base + end] = body
        put(raw, base + 14 + 2 * slot, begin, 2)
        put(raw, base + 12, slot + 1, 2)
        put(raw, base + 2, begin - 14 - 2 * (slot + 1), 2)
        row = page * 256 + slot
        assert row not in all_rows
        all_rows[row] = requested
        placements.append({'id': requested[0], 'page': page, 'slot': slot, 'ROW': row,
                           'record_hex': body.hex()})
    final_members = []
    allocations = []
    for ordinal, physical in enumerate(before['physical']):
        entries = sorted(reference.key(values[ordinal], row) for row, values in all_rows.items())
        page = physical['root']
        source = before['raw'][page * PAGE:(page + 1) * PAGE]
        seed = reference.n(source, 24, 2)
        if framing(entries, seed)[1] <= CAPACITY:
            raw[page * PAGE:(page + 1) * PAGE] = leaf(source, entries, seed, definition)
            members = {page}
        else:
            middle = len(entries) // 2
            right = len(raw) // PAGE
            left = right + 1
            raw.extend(bytes(2 * PAGE))
            raw[right * PAGE:(right + 1) * PAGE] = leaf(None, entries[middle:], 0, definition, left, 0)
            raw[left * PAGE:(left + 1) * PAGE] = leaf(None, entries[:middle], 0, definition, 0, right)
            raw[page * PAGE:(page + 1) * PAGE] = root(source, entries[middle - 1], left, right, definition)
            members = {page, left, right}
            allocations.extend([{'physical': ordinal, 'purpose': 'INDEX', 'page': right},
                                {'physical': ordinal, 'purpose': 'INDEX', 'page': left}])
        final_members.append(members)
    changes = {'owned': before['maps']['owned']['members'] | set(new_data),
               'hint': hints,
               'global': before['maps']['global']['members'] - set(range(len(before['raw']) // PAGE, len(raw) // PAGE))}
    for index, members in enumerate(final_members):
        changes['index' + str(index)] = members
    for role, wanted in changes.items():
        metadata = before['maps'][role]
        for member in metadata['members'] ^ wanted:
            at, mask = metadata['locations'][member]
            raw[at] ^= mask
    put(raw, definition * PAGE + 16, len(all_rows))
    original_families = {value[1] for value in before['rows'].values()}
    final_families = {value[1] for value in all_rows.values()}
    deltas = [len(placements), len(final_families - original_families)]
    for physical, delta in zip(before['physical'], deltas, strict=True):
        put(raw, physical['history'], reference.n(before['raw'], physical['history']) + delta)
    # Independently decode the model's new frames and both full key/ROW graphs.
    for placement, requested in zip(placements, reference.requests(case['variant']), strict=True):
        begin, end = reference.wire.frame(raw, placement['page'], placement['slot'])
        assert raw[begin:end] == reference.record(requested)
    for ordinal, physical in enumerate(before['physical']):
        nodes, _, entries = reference.tree(bytes(raw), physical['root'], definition)
        assert set(nodes) == final_members[ordinal]
        assert entries == sorted(reference.key(value[ordinal], row) for row, value in all_rows.items())
    return bytes(raw), {'placements': placements, 'new_data': new_data, 'allocations': allocations,
                        'counter_deltas': deltas, 'rows': len(all_rows),
                        'image_policy': 'Independent SOURCE+REQUEST engineering bytes; not public AFTER equality.'}
