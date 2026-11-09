"""Prospective source/request-only engineering child-split image, no source calls."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as SOURCE
from parent_index_split_originals import wire as DECODE

PAGE = 4096
EXTRA = [2147483647 - position * 65536 for position in range(225)]
ORIGINAL = list(range(-226, 0)) + list(range(1, 227))


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


def derive(state, identity, binary):
    original, table, layout, maps, nodes, leaves = state
    assert identity == 227 and identity not in table['rows']
    assert set(table['rows']) == set(ORIGINAL + EXTRA)
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == 677
    root = layout['root']
    assert len(nodes) == 3 and len(leaves) == 2
    peer, target = leaves
    assert nodes[root]['height'] == 1 and nodes[root]['children'] == [peer]
    assert nodes[root]['tail'] == target and len(nodes[root]['entries']) == 1
    assert DECODE.key(nodes[root]['entries'][0]) == -1
    assert len(nodes[peer]['entries']) == 226
    assert len(nodes[target]['entries']) == 451
    assert nodes[target]['prefix'] == 1 and nodes[target]['free'] == 7
    for old_id, row in table['rows'].items():
        begin, end = SOURCE.frame(original, row['page'], row['slot'])
        value = row['values'][1]
        if value == 'NULL':
            stored = None
        else:
            assert value.startswith('Binary:')
            stored = bytes.fromhex(value.removeprefix('Binary:'))
        assert row['values'][0] == 'Integer:' + str(old_id)
        assert original[begin:end] == record(old_id, stored)
        assert original[begin:end].hex() == table['raw_rows'][old_id]
    image = bytearray(original)
    original_eof = len(original) // PAGE
    new_record = record(identity, binary)
    highest = max(maps['hint']['members'])
    base = highest * PAGE
    count = int.from_bytes(original[base + 12:base + 14], 'little')
    free = int.from_bytes(original[base + 2:base + 4], 'little')
    end = PAGE if count == 0 else int.from_bytes(original[base + 14 + 2 * (count - 1):base + 16 + 2 * (count - 1)], 'little') & 0x1fff
    assert free == end - (14 + 2 * count)
    hints = set(maps['hint']['members'])
    data_pages = set()
    if count == 255 or len(new_record) + 2 > free:
        hints.remove(highest)
        highest = original_eof
        data_pages.add(highest)
        hints.add(highest)
        image.extend(bytes(PAGE))
        base = highest * PAGE
        image[base:base + 2] = bytes([1, 1])
        put(image, base + 4, layout['definition'], 4)
        count, end = 0, PAGE
    start = end - len(new_record)
    assert start >= 14 + 2 * (count + 1) and count < 255
    image[base + start:base + end] = new_record
    put(image, base + 14 + 2 * count, start)
    put(image, base + 12, count + 1)
    put(image, base + 2, start - (14 + 2 * (count + 1)))
    reference = highest << 8 | count
    target_entries = sorted(nodes[target]['entries'] + [key(identity, reference)])
    assert len(target_entries) == 452 and len(set(target_entries)) == 452
    assert common(target_entries) == 1 and 452 * 9 - 451 == 3617
    half = len(target_entries) // 2
    left_entries, retained_entries = target_entries[:half], target_entries[half:]
    assert half == 226 and DECODE.key(left_entries[-1]) == 226
    new_left = len(image) // PAGE
    image.extend(render(None, left_entries, None, 0, layout['definition'], peer, target))
    target_source = original[target * PAGE:(target + 1) * PAGE]
    image[target * PAGE:(target + 1) * PAGE] = render(
        target_source, retained_entries, None, 0, layout['definition'], new_left, 0)
    # The original left peer receives no re-encoding or padding normalization.
    put(image, peer * PAGE + 16, new_left, 4)
    root_source = original[root * PAGE:(root + 1) * PAGE]
    bounds = [nodes[peer]['entries'][-1], left_entries[-1]]
    image[root * PAGE:(root + 1) * PAGE] = render(
        root_source, bounds, [peer, new_left], target, layout['definition'])
    desired = {'owned': maps['owned']['members'] | data_pages,
               'hint': hints,
               'index_owned': maps['index_owned']['members'] | {new_left},
               'global': maps['global']['members'] - data_pages - {new_left}}
    changes = {}
    for role, members in desired.items():
        for number in maps[role]['members'] ^ members:
            offset, mask = maps[role]['locations'][number]
            old = changes.get(offset, original[offset])
            changes[offset] = old | mask if number in members else old & ~mask
    for at, value in changes.items():
        image[at] = value
    for role in ['table_count', 'history']:
        assert int.from_bytes(original[layout[role]:layout[role] + 4], 'little') == 677
        put(image, layout[role], 678, 4)
    decoded, actual_leaves, actual_entries = DECODE.tree(image, root, layout['definition'])
    references = {(row['page'] << 8) | row['slot']: old_id for old_id, row in table['rows'].items()}
    references[reference] = identity
    assert len(references) == 678
    assert actual_entries == sorted(key(old_id, old_ref) for old_ref, old_id in references.items())
    assert actual_leaves == [peer, new_left, target]
    assert decoded[root]['entries'] == bounds and decoded[root]['children'] == [peer, new_left]
    assert decoded[root]['tail'] == target and decoded[root]['height'] == 1
    assert decoded[new_left]['prefix'] == 0 and decoded[target]['prefix'] == 1
    assert image[new_left * PAGE + 2514:(new_left + 1) * PAGE] == bytes(1582)
    for role, members in desired.items():
        assert SOURCE.map_record(image, layout[role])['members'] == members
    for old_id, row in table['rows'].items():
        begin, end = SOURCE.frame(image, row['page'], row['slot'])
        assert image[begin:end].hex() == table['raw_rows'][old_id]
    begin, end = SOURCE.frame(image, highest, count)
    assert image[begin:end] == new_record
    return bytes(image), {'source_rows': 677, 'final_rows': 678, 'old_ROWS_preserved': 677,
                          'new_ROW': reference, 'new_page': highest, 'new_slot': count,
                          'new_record_bytes': len(new_record), 'new_DATA_pages': sorted(data_pages),
                          'new_INDEX_LEFT': new_left, 'retained_RIGHT': target, 'left_peer': peer,
                          'root': root, 'root_height': 1, 'root_bounds': [entry.hex() for entry in bounds],
                          'root_children': [peer, new_left], 'root_tail': target,
                          'leaf_entries': [226, 226, 226], 'fresh_prefix': 0, 'retained_prefix': 1,
                          'table_counter': [677, 678], 'historical_counter': [677, 678],
                          'bytes_added': len(image) - len(original),
                          'map_byte_effects': [{'offset': at, 'before': original[at], 'after': value}
                                               for at, value in sorted(changes.items())]}
