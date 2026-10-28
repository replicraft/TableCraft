"""Prospective source/request-only general height-one INSERT image. No fixture calls."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as SOURCE
from parent_index_split_originals import wire as DECODE

PAGE = 4096
PRIOR = list(range(-226, 0)) + list(range(1, 228)) + [2147483647 - position * 65536 for position in range(225)]
NEGATIVE = [-2147483648 + position * 65536 for position in range(225)]
GAPS = [2147483647 - position * 65536 - 32768 for position in range(225)]


def put(raw, at, value, width=2):
    raw[at:at + width] = value.to_bytes(width, 'little')


def key(value, reference):
    return (bytes([127]) + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
            + reference.to_bytes(4, 'big'))


def record(value, binary):
    assert binary is None or len(binary) <= 255
    body = bytes([2, 0]) + value.to_bytes(4, 'little', signed=True)
    if binary is not None:
        body += binary
    return (body + len(body).to_bytes(2, 'little') + bytes([6, 0, 1, 0,
                                                          1 if binary is None else 3]))


def common(entries):
    result = 9
    for entry in entries[1:]:
        equal = 0
        while equal < result and entries[0][equal] == entry[equal]:
            equal += 1
        result = equal
    return result


def render(original, entries, children, tail, definition, previous=0, next_=0):
    node = children is not None
    assert entries and all(len(entry) == 9 for entry in entries)
    seed = 0 if original is None else int.from_bytes(original[24:26], 'little')
    best = common(entries)
    prefix = min(seed, best)
    full = len(entries) * (13 if node else 9)
    active = full - (len(entries) - 1) * prefix
    if active > 3616:
        prefix = best
        active = full - (len(entries) - 1) * prefix
    assert active <= 3616
    page = bytearray(PAGE) if original is None else bytearray(original)
    page[:2] = bytes([3 if node else 4, 1])
    put(page, 2, 3616 - active)
    put(page, 4, definition, 4)
    put(page, 12, previous, 4)
    put(page, 16, next_, 4)
    put(page, 20, tail, 4)
    put(page, 24, prefix)
    page[26] = 1 if node else 0
    page[27:480] = bytes(453)
    end = 0
    for position, entry in enumerate(entries):
        suffix = entry if position == 0 else entry[prefix:]
        page[480 + end:480 + end + len(suffix)] = suffix
        end += len(suffix)
        if node:
            page[480 + end:480 + end + 4] = children[position].to_bytes(4, 'big')
            end += 4
        page[27 + end // 8] |= 1 << (end % 8)
    assert end == active
    if original is None:
        assert page[480 + active:] == bytes(3616 - active)
    else:
        assert page[480 + active:] == original[480 + active:]
    return page


def capacity(entries, seed):
    """Independent complete key/ROW leaf measurement; no builder decisions."""
    assert entries and all(len(entry) == 9 for entry in entries)
    best = common(entries)
    chosen = min(seed, best)
    size = len(entries) * 9 - (len(entries) - 1) * chosen
    if size > 3616:
        chosen = best
        size = len(entries) * 9 - (len(entries) - 1) * chosen
    return chosen, size


def normalize(original, nodes, leaves, requested):
    """Original-child order, then stable local RIGHT/LEFT creation worklist."""
    threshold = [nodes[number]['entries'][-1] for number in leaves[:-1]]
    pending = [[] for _ in leaves]
    for entry in requested:
        position = next((index for index, bound in enumerate(threshold)
                         if entry <= bound), len(leaves) - 1)
        pending[position].append(entry)
    symbols = []
    creation = []
    logical = []
    for number, extra in zip(leaves, pending):
        seed = nodes[number]['prefix']
        index = len(symbols)
        symbols.append({'original': number, 'seed': seed,
                        'entries': sorted(nodes[number]['entries'] + extra)})
        work = [index]
        local = [index]

        def divide(current):
            entries = symbols[current]['entries']
            half = len(entries) // 2
            left, right = entries[:half], entries[half:]
            symbols[current]['entries'] = right
            added = len(symbols)
            symbols.append({'original': None, 'seed': 0, 'entries': left})
            creation.append(added)
            local.insert(local.index(current), added)
            work.append(added)

        if capacity(symbols[index]['entries'], seed)[1] > 3616:
            divide(index)
        while True:
            changed = False
            cursor = 0
            while cursor < len(work):
                current = work[cursor]
                if capacity(symbols[current]['entries'], symbols[current]['seed'])[1] > 3616:
                    divide(current)
                    changed = True
                cursor += 1
            if not changed:
                break
        logical.extend(local)
    assert len(set(creation)) == len(creation)
    assert len(logical) == len(symbols)
    assert set(logical) == set(range(len(symbols)))
    return symbols, creation, logical


def append_record(image, layout, hints, value, binary):
    """Highest actual candidate once, then EOF; normal visible slots only."""
    payload = record(value, binary)
    number = max(hints)
    base = number * PAGE
    count = int.from_bytes(image[base + 12:base + 14], 'little')
    free = int.from_bytes(image[base + 2:base + 4], 'little')
    end = PAGE if count == 0 else int.from_bytes(
        image[base + 12 + 2 * count:base + 14 + 2 * count], 'little') & 0x1fff
    assert free == end - (14 + 2 * count)
    added = None
    if count == 255 or len(payload) + 2 > free:
        hints.remove(number)
        number = len(image) // PAGE
        added = number
        hints.add(number)
        image.extend(bytes(PAGE))
        base = number * PAGE
        image[base:base + 2] = bytes([1, 1])
        put(image, base + 4, layout['definition'], 4)
        count, end = 0, PAGE
    start = end - len(payload)
    assert count < 255 and start >= 16 + 2 * count
    image[base + start:base + end] = payload
    put(image, base + 14 + 2 * count, start)
    put(image, base + 12, count + 1)
    put(image, base + 2, start - (16 + 2 * count))
    return number << 8 | count, added


def derive(state, requests):
    """Derive solely from the eligible immutable BEFORE state and closed rows.

    This prospective profile requires all future map cells already represented.
    It does not infer metadata growth or borrow any public AFTER page bytes.
    """
    original, table, layout, maps, nodes, leaves = state
    assert len(original) % PAGE == 0
    assert [value for value, _ in requests] == [-227, 228]
    assert all(binary is not None and len(binary) in (0, 84) for _, binary in requests)
    assert len({len(binary) for _, binary in requests}) == 1
    for value, binary in requests:
        assert binary == bytes((position * 31 + value) & 255 for position in range(len(binary)))
    assert set(table['rows']) == set(PRIOR + NEGATIVE + GAPS)
    assert len(PRIOR) == 678 and len(set(PRIOR + NEGATIVE + GAPS)) == 1128
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == 1128
    root = layout['root']
    assert len(nodes) == 4 and len(leaves) == 3
    assert nodes[root]['height'] == 1
    assert nodes[root]['children'] == leaves[:-1]
    assert nodes[root]['tail'] == leaves[-1]
    assert [DECODE.key(entry) for entry in nodes[root]['entries']] == [-1, 226]
    assert [len(nodes[number]['entries']) for number in leaves] == [451, 226, 451]
    for number in [leaves[0], leaves[-1]]:
        assert nodes[number]['prefix'] == 1 and nodes[number]['free'] == 7
    for old_id, row in table['rows'].items():
        begin, end = SOURCE.frame(original, row['page'], row['slot'])
        value = row['values'][1]
        stored = None if value == 'NULL' else bytes.fromhex(value.removeprefix('Binary:'))
        assert value == 'NULL' or value.startswith('Binary:')
        assert row['values'][0] == 'Integer:' + str(old_id)
        assert original[begin:end] == record(old_id, stored)
        assert original[begin:end].hex() == table['raw_rows'][old_id]
    image = bytearray(original)
    hints = set(maps['hint']['members'])
    data_pages = set()
    references = []
    for value, binary in requests:
        assert value not in table['rows']
        reference, added = append_record(image, layout, hints, value, binary)
        references.append(reference)
        if added is not None:
            data_pages.add(added)
    requested = [key(value, reference) for (value, _), reference in zip(requests, references)]
    symbols, creation, logical = normalize(original, nodes, leaves, requested)
    assert len(creation) == 2 and len(logical) == 5
    # Preflight full final parent framing before any symbolic INDEX allocation.
    bounds = [symbols[index]['entries'][-1] for index in logical[:-1]]
    assert len(bounds) == 4
    root_source = original[root * PAGE:(root + 1) * PAGE]
    seed = int.from_bytes(root_source[24:26], 'little')
    chosen = min(seed, common(bounds))
    active = 13 * len(bounds) - (len(bounds) - 1) * chosen
    if active > 3616:
        chosen = common(bounds)
        active = 13 * len(bounds) - (len(bounds) - 1) * chosen
    assert active <= 3616
    assigned = {}
    for index in creation:
        assigned[index] = len(image) // PAGE
        image.extend(bytes(PAGE))
    numbers = [symbols[index]['original'] if symbols[index]['original'] is not None
               else assigned[index] for index in logical]
    for position, index in enumerate(logical):
        symbol = symbols[index]
        number = numbers[position]
        source = None if symbol['original'] is None else original[number * PAGE:(number + 1) * PAGE]
        page = render(source, symbol['entries'], None, 0, layout['definition'],
                      numbers[position - 1] if position else 0,
                      numbers[position + 1] if position + 1 < len(numbers) else 0)
        image[number * PAGE:(number + 1) * PAGE] = page
    image[root * PAGE:(root + 1) * PAGE] = render(
        root_source, bounds, numbers[:-1], numbers[-1], layout['definition'])
    index_pages = set(assigned.values())
    desired = {'owned': maps['owned']['members'] | data_pages,
               'hint': hints,
               'index_owned': maps['index_owned']['members'] | index_pages,
               'global': maps['global']['members'] - data_pages - index_pages}
    changes = {}
    for role, members in desired.items():
        for number in maps[role]['members'] ^ members:
            assert number in maps[role]['locations'], (role, number, 'unrepresented cell')
            offset, mask = maps[role]['locations'][number]
            value = changes.get(offset, original[offset])
            changes[offset] = value | mask if number in members else value & ~mask
    for offset, value in changes.items():
        image[offset] = value
    for role in ['table_count', 'history']:
        assert int.from_bytes(original[layout[role]:layout[role] + 4], 'little') == 1128
        put(image, layout[role], 1130, 4)
    decoded, final_leaves, entries = DECODE.tree(image, root, layout['definition'])
    all_refs = {(row['page'] << 8) | row['slot']: value for value, row in table['rows'].items()}
    all_refs.update({reference: value for (value, _), reference in zip(requests, references)})
    assert len(all_refs) == 1130
    assert entries == sorted(key(value, reference) for reference, value in all_refs.items())
    assert final_leaves == numbers and len(decoded) == 6
    assert decoded[root]['entries'] == bounds
    assert decoded[root]['children'] == numbers[:-1]
    assert decoded[root]['tail'] == numbers[-1]
    assert [len(decoded[number]['entries']) for number in numbers] == [226, 226, 226, 226, 226]
    for role, members in desired.items():
        assert SOURCE.map_record(image, layout[role])['members'] == members
    for old_id, row in table['rows'].items():
        begin, end = SOURCE.frame(image, row['page'], row['slot'])
        assert image[begin:end].hex() == table['raw_rows'][old_id]
    for (value, binary), reference in zip(requests, references):
        begin, end = SOURCE.frame(image, reference >> 8, reference & 255)
        assert image[begin:end] == record(value, binary)
    return bytes(image), {
        'source_rows': 1128, 'final_rows': 1130, 'old_ROWS_preserved': 1128,
        'new_ROWS': references, 'new_DATA_pages': sorted(data_pages),
        'new_INDEX_creation_order': [assigned[index] for index in creation],
        'logical_leaves': numbers, 'leaf_entries': [len(decoded[number]['entries']) for number in numbers],
        'root': root, 'root_height': 1, 'root_tail': numbers[-1],
        'root_bounds': [entry.hex() for entry in bounds], 'root_children': numbers[:-1],
        'table_counter': [1128, 1130], 'historical_counter': [1128, 1130],
        'bytes_added': len(image) - len(original),
        'map_byte_effects': [{'offset': offset, 'before': original[offset], 'after': value}
                             for offset, value in sorted(changes.items())],
    }
