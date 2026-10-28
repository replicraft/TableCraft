"""Full engineering images from immutable source state and explicit INSERT only."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as SOURCE
from parent_index_split_originals import wire as DECODE

PAGE = 4096
CAPACITY = 3616
PAYLOAD = 480
MASK = 27


def word(bytes_, offset, value, width=2):
    bytes_[offset:offset + width] = value.to_bytes(width, 'little')


def record(identity, binary):
    assert -(1 << 31) <= identity < 1 << 31
    assert binary is None or len(binary) <= 255
    body = bytes([2, 0]) + identity.to_bytes(4, 'little', signed=True)
    if binary is not None:
        body += binary
    return (body + len(body).to_bytes(2, 'little') + bytes([6, 0, 1, 0,
                                                           1 if binary is None else 3]))


def leaf(definition, entries, previous, next_):
    assert len(entries) == 226 and all(len(entry) == 9 for entry in entries)
    # Fresh seed zero fits: no public compressor or production serializer used.
    active = sum(map(len, entries))
    assert active == 2034 <= CAPACITY
    page = bytearray(PAGE)
    page[:2] = bytes([4, 1])
    word(page, 2, CAPACITY - active)
    word(page, 4, definition, 4)
    word(page, 12, previous, 4)
    word(page, 16, next_, 4)
    end = 0
    for entry in entries:
        page[PAYLOAD + end:PAYLOAD + end + 9] = entry
        end += 9
        page[MASK + end // 8] |= 1 << (end % 8)
    assert page[PAYLOAD + active:] == bytes(PAGE - PAYLOAD - active)
    return page


def derive(state, identity, binary):
    original, table, layout, maps, nodes, leaves = state
    root = layout['root']
    assert nodes[root]['height'] == 0 and len(nodes) == len(leaves) == 1
    assert identity not in table['rows'] and len(table['rows']) == 451
    new_record = record(identity, binary)
    image = bytearray(original)
    eof = len(image) // PAGE
    highest = max(maps['hint']['members'])
    original_page = original[highest * PAGE:(highest + 1) * PAGE]
    count = int.from_bytes(original_page[12:14], 'little')
    free = int.from_bytes(original_page[2:4], 'little')
    end = PAGE if count == 0 else int.from_bytes(
        original_page[14 + 2 * (count - 1):16 + 2 * (count - 1)], 'little') & 0x1fff
    assert free == end - (14 + 2 * count)
    hint_members = set(maps['hint']['members'])
    added_data = []
    if count < 255 and len(new_record) + 2 <= free:
        data_number = highest
    else:
        hint_members.remove(highest)
        data_number = eof
        added_data.append(data_number)
        image.extend(bytes(PAGE))
        base = data_number * PAGE
        image[base:base + 2] = bytes([1, 1])
        word(image, base + 4, layout['definition'], 4)
        count, end = 0, PAGE
        hint_members.add(data_number)
    base = data_number * PAGE
    start = end - len(new_record)
    assert start >= 14 + 2 * (count + 1) and count < 255
    image[base + start:base + end] = new_record
    word(image, base + 14 + 2 * count, start)
    word(image, base + 12, count + 1)
    word(image, base + 2, start - (14 + 2 * (count + 1)))
    reference = data_number << 8 | count

    references = {(row['page'] << 8) | row['slot']: value
                  for value, row in table['rows'].items()}
    references[reference] = identity
    entries = sorted(SOURCE.int_key(value, ref) for ref, value in references.items())
    assert len(entries) == 452 and entries == sorted(set(entries))
    left_entries, right_entries = entries[:226], entries[226:]
    right = len(image) // PAGE
    left = right + 1
    image.extend(leaf(layout['definition'], right_entries, left, 0))
    image.extend(leaf(layout['definition'], left_entries, 0, right))

    old_root = original[root * PAGE:(root + 1) * PAGE]
    root_page = bytearray(old_root)
    root_page[0] = 3
    root_page[26] = 1
    word(root_page, 2, 3603)
    word(root_page, 12, 0, 4)
    word(root_page, 16, 0, 4)
    word(root_page, 20, right, 4)
    word(root_page, 24, 0)
    root_page[27:480] = bytes(453)
    root_page[28] = 1 << 5
    root_page[480:493] = left_entries[-1] + left.to_bytes(4, 'big')
    assert root_page[493:] == old_root[493:]
    image[root * PAGE:(root + 1) * PAGE] = root_page

    desired = {'global': maps['global']['members'] - set(added_data) - {right, left},
               'owned': maps['owned']['members'] | set(added_data),
               'hint': hint_members,
               'index_owned': maps['index_owned']['members'] | {right, left}}
    changes = {}
    for role, members in desired.items():
        for number in maps[role]['members'] ^ members:
            offset, mask = maps[role]['locations'][number]
            before = changes.get(offset, original[offset])
            changes[offset] = before | mask if number in members else before & ~mask
    for offset, value in changes.items():
        image[offset] = value
    for key in ['table_count', 'history']:
        before = int.from_bytes(original[layout[key]:layout[key] + 4], 'little')
        assert before == 451
        word(image, layout[key], before + 1, 4)

    # Decode the complete independently encoded graph and every original/new ROW.
    decoded, actual_leaves, stream = DECODE.tree(image, root, layout['definition'])
    assert stream == entries and actual_leaves == [left, right]
    assert decoded[root]['entries'] == [left_entries[-1]]
    assert decoded[root]['children'] == [left] and decoded[root]['tail'] == right
    for value, old_row in table['rows'].items():
        begin, stop = SOURCE.frame(image, old_row['page'], old_row['slot'])
        assert image[begin:stop].hex() == table['raw_rows'][value]
    begin, stop = SOURCE.frame(image, data_number, count)
    assert image[begin:stop] == new_record
    for role, members in desired.items():
        assert SOURCE.map_record(image, layout[role])['members'] == members
    facts = {'source_pages': eof, 'final_pages': len(image) // PAGE,
             'new_ID': identity, 'binary_bytes': None if binary is None else len(binary),
             'new_record_bytes': len(new_record), 'new_ROW': reference,
             'new_page': data_number, 'new_slot': count,
             'DATA_pages': added_data, 'RIGHT_leaf': right, 'LEFT_leaf': left,
             'root': root, 'root_height': 1, 'separator_key_ROW_hex': left_entries[-1].hex(),
             'leaf_populations': [226, 226], 'fresh_leaf_prefixes': [0, 0],
             'old_root_residue_from': root * PAGE + 493,
             'table_counter': [451, 452], 'historical_counter': [451, 452],
             'complete_key_ROW_entries': len(entries), 'old_ROWS_preserved': len(table['rows']),
             'map_byte_effects': [{'offset': n, 'before': original[n], 'after': v}
                                  for n, v in sorted(changes.items())]}
    return bytes(image), facts
