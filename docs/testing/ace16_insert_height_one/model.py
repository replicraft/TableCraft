"""Source/request-only fitted INSERT into a checked original two-leaf tree."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as SOURCE
from parent_index_split_originals import wire as DECODE

PAGE = 4096
ENTRY_START = 480
ENTRY_MASK = 27


def put(raw, offset, value, width=2):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def key(value, reference):
    return (bytes([127]) + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
            + reference.to_bytes(4, 'big'))


def record(value, binary):
    assert -(1 << 31) <= value < 1 << 31
    assert binary is None or len(binary) <= 255
    body = bytes([2, 0]) + value.to_bytes(4, 'little', signed=True)
    if binary is not None:
        body += binary
    return (body + len(body).to_bytes(2, 'little') + bytes([6, 0, 1, 0,
                                                          1 if binary is None else 3]))


def derive(state, identity, binary):
    original, table, layout, maps, nodes, leaves = state
    assert len(table['rows']) == 452 and identity == 227 and identity not in table['rows']
    assert len(nodes) == 3 and len(leaves) == 2
    root, left, right = layout['root'], leaves[0], leaves[1]
    assert nodes[root]['height'] == 1 and len(nodes[root]['entries']) == 1
    assert nodes[root]['children'] == [left] and nodes[root]['tail'] == right
    assert len(nodes[left]['entries']) == len(nodes[right]['entries']) == 226
    assert nodes[left]['prefix'] == nodes[right]['prefix'] == 0

    highest = max(maps['hint']['members'])
    base = highest * PAGE
    count = int.from_bytes(original[base + 12:base + 14], 'little')
    free = int.from_bytes(original[base + 2:base + 4], 'little')
    end = int.from_bytes(original[base + 14 + 2 * (count - 1):base + 16 + 2 * (count - 1)], 'little') & 0x1fff
    new_record = record(identity, binary)
    assert count > 0 and count < 255 and free == end - (14 + 2 * count)
    assert len(new_record) + 2 <= free
    start = end - len(new_record)
    reference = highest << 8 | count
    image = bytearray(original)
    image[base + start:base + end] = new_record
    put(image, base + 14 + 2 * count, start)
    put(image, base + 12, count + 1)
    put(image, base + 2, start - (14 + 2 * (count + 1)))

    entries = list(nodes[right]['entries']) + [key(identity, reference)]
    assert entries == sorted(set(entries)) and len(entries) == 227
    active = len(entries) * 9
    assert active == 2043 <= 3616
    page = bytearray(original[right * PAGE:(right + 1) * PAGE])
    assert page[26] == 0 and page[:2] == bytes([4, 1])
    assert int.from_bytes(page[24:26], 'little') == 0
    # Seed zero remains zero because all 227 complete wires fit. Rebuild only
    # active framing, retaining every original unused body byte beyond it.
    page[ENTRY_MASK:ENTRY_START] = bytes(453)
    end = 0
    for entry in entries:
        page[ENTRY_START + end:ENTRY_START + end + 9] = entry
        end += 9
        page[ENTRY_MASK + end // 8] |= 1 << (end % 8)
    put(page, 2, 3616 - active)
    assert page[ENTRY_START + active:] == original[right * PAGE + ENTRY_START + active:(right + 1) * PAGE]
    image[right * PAGE:(right + 1) * PAGE] = page
    for name in ['table_count', 'history']:
        value = int.from_bytes(original[layout[name]:layout[name] + 4], 'little')
        assert value == 452
        put(image, layout[name], value + 1, 4)

    decoded, actual_leaves, stream = DECODE.tree(image, root, layout['definition'])
    references = {(row['page'] << 8) | row['slot']: value for value, row in table['rows'].items()}
    references[reference] = identity
    assert len(references) == 453 and actual_leaves == leaves
    assert stream == sorted(key(value, row) for row, value in references.items())
    assert decoded[root]['entries'] == nodes[root]['entries']
    assert decoded[root]['children'] == [left] and decoded[root]['tail'] == right
    for page_number in [root, left]:
        assert image[page_number * PAGE:(page_number + 1) * PAGE] == original[page_number * PAGE:(page_number + 1) * PAGE]
    for number in nodes:
        assert (decoded[number]['previous'], decoded[number]['next']) == (nodes[number]['previous'], nodes[number]['next'])
    for old_id, row in table['rows'].items():
        begin, stop = SOURCE.frame(image, row['page'], row['slot'])
        assert image[begin:stop].hex() == table['raw_rows'][old_id]
    begin, stop = SOURCE.frame(image, highest, count)
    assert image[begin:stop] == new_record
    for role in ['global', 'owned', 'hint', 'index_owned']:
        assert SOURCE.map_record(image, layout[role]) == maps[role]
    assert image[:PAGE] == original[:PAGE] and len(image) == len(original)
    facts = {'source_rows': 452, 'final_rows': 453, 'old_ROWS_preserved': 452,
             'new_ID': identity, 'binary_bytes': None if binary is None else len(binary),
             'new_record_bytes': len(new_record), 'new_ROW': reference,
             'new_page': highest, 'new_slot': count, 'DATA_free_before': free,
             'DATA_free_after': start - (14 + 2 * (count + 1)),
             'root': root, 'root_height': 1, 'root_bound_key_ROW': nodes[root]['entries'][0].hex(),
             'LEFT': left, 'RIGHT': right, 'leaf_entries': [226, 227],
             'right_prefix': 0, 'right_active': 2043, 'right_free': 1573,
             'original_right_residue_from': right * PAGE + 2523,
             'source_right_residue_bytes_preserved': 1573,
             'table_counter': [452, 453], 'historical_counter': [452, 453],
             'new_pages': 0, 'map_byte_effects': [], 'bytes_added': 0,
             'source_header_root_left_and_siblings_unchanged': True}
    return bytes(image), facts
