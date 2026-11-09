"""First-party raw DROP hypotheses, not an Access or TableCraft native writer.

Reuse the maintained literal raw owners for catalog/LVAL placement. DROP TD and
MR2 framing below are independent of production codecs. Never compact native IDs.
"""
import importlib.util
import struct

PAGE = 4096
DEFINITION = 7
PROPERTY = 10
LENGTH = 8
FREE_BYTES = 2
DEFINITION_FOOTER_BYTES = 8
NEXT_PAGE = 4
MAX_COLUMNS = 41
VARIABLE_SLOTS = 43
ACTIVE_COLUMNS = 45
PHYSICAL_INDEXES = 51
COLUMN_BLOCK = 63
INDEX_ENTRY_BYTES = 12
COLUMN_BYTES = 25
NATIVE_NUMBER = 5
STORED_ID = 9
NAME_LIST = 128
COLUMN_GROUP = 1


def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]


def u32(data, at):
    return struct.unpack_from('<I', data, at)[0]


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    return owner


def declarations(page):
    count = u16(page, ACTIVE_COLUMNS)
    start = COLUMN_BLOCK + u32(page, PHYSICAL_INDEXES) * INDEX_ENTRY_BYTES
    position = start + count * COLUMN_BYTES
    fields = []
    for index in range(count):
        entry = page[start + index * COLUMN_BYTES:start + (index + 1) * COLUMN_BYTES]
        size = u16(page, position)
        finish = position + 2 + size
        assert size % 2 == 0 and finish <= u32(page, LENGTH)
        fields.append((entry, page[position:finish], page[position + 2:finish].decode('utf-16le')))
        position = finish
    return start, position, fields


def remove_definition(image, selected):
    page = image[DEFINITION * PAGE:(DEFINITION + 1) * PAGE]
    assert len(page) == PAGE and u32(page, NEXT_PAGE) == 0
    assert u32(page, PHYSICAL_INDEXES) == 1
    length = u32(page, LENGTH)
    assert length <= PAGE and page[length:] == bytes(PAGE - length)
    start, tail, fields = declarations(page)
    removed = [field for field in fields if field[2] == selected]
    assert len(removed) == 1 and u16(removed[0][0], NATIVE_NUMBER) != 0
    kept = [field for field in fields if field[2] != selected]
    assert kept and kept[0][2] == 'ID' and u16(kept[0][0], NATIVE_NUMBER) == 0
    # Index descriptors, column order and LVAL map suffix are moved intact.
    result = bytearray(page[:start] + b''.join(field[0] for field in kept)
                       + b''.join(field[1] for field in kept) + page[tail:length])
    struct.pack_into('<H', result, ACTIVE_COLUMNS, len(kept))
    struct.pack_into('<I', result, LENGTH, len(result))
    struct.pack_into('<H', result, FREE_BYTES, PAGE - len(result) - DEFINITION_FOOTER_BYTES)
    result.extend(bytes(PAGE - len(result)))
    assert result[MAX_COLUMNS:ACTIVE_COLUMNS] == page[MAX_COLUMNS:ACTIVE_COLUMNS]
    _, new_tail, actual = declarations(result)
    assert result[new_tail:u32(result, LENGTH)] == page[tail:length]
    assert actual == kept
    return result, {
        'removed_native_number': u16(removed[0][0], NATIVE_NUMBER),
        'removed_stored_id': u16(removed[0][0], STORED_ID),
        'before_active_count': len(fields), 'after_active_count': len(kept),
        'max_columns': u16(page, MAX_COLUMNS), 'variable_slots': u16(page, VARIABLE_SLOTS),
        'retained_index_prefix_exact': result[COLUMN_BLOCK:start] == page[COLUMN_BLOCK:start],
        'retained_definition_suffix_exact': True,
        'surviving_names': [field[2] for field in kept],
    }


def remove_group(body, selected):
    assert body[:4] == b'MR2\0' and u16(body, 8) == NAME_LIST
    dictionary_end = 4 + u32(body, 4)
    position = dictionary_end
    groups = []
    removed = []
    while position < len(body):
        length = u32(body, position)
        assert length >= 12 and position + length <= len(body)
        name_bytes = u16(body, position + 10)
        assert u32(body, position + 6) == name_bytes + 6
        assert name_bytes % 2 == 0 and 12 + name_bytes <= length
        name = body[position + 12:position + 12 + name_bytes].decode('utf-16le')
        block = body[position:position + length]
        if u16(body, position + 4) == COLUMN_GROUP and name == selected:
            removed.append(block)
        else:
            groups.append(block)
        position += length
    assert position == len(body) and len(removed) == 1
    # Deliberately retain the original dictionary: unused-key acceptance is an
    # explicit hypothesis to observe, not a relaxation of native writer framing.
    return body[:dictionary_end] + b''.join(groups)


def prediction(before, selected, variant, repo):
    assert len(before) == 11 * PAGE and selected.isascii()
    rename = load(repo / 'docs/testing/saved_field_rename/model.py', 'drop_role_observer')
    add = load(repo / 'docs/testing/short_text_add/model.py', 'drop_raw_placement')
    body_start, body_end, _, old_reference = rename.live_user_body(before)
    definition, facts = remove_definition(before, selected)
    after = bytearray(before)
    after[DEFINITION * PAGE:(DEFINITION + 1) * PAGE] = definition
    if variant == 'append-live-group-removed':
        body = remove_group(before[body_start:body_end], selected)
        reference = add.append_body(before, after, body)
        after[4 * PAGE:5 * PAGE] = add.parent_page(before, body, reference)
    else:
        assert variant == 'td-only'
        reference = old_reference
    changed = [page for page in range(11)
               if before[page * PAGE:(page + 1) * PAGE] != after[page * PAGE:(page + 1) * PAGE]]
    assert set(changed) <= ({7} if variant == 'td-only' else {4, 7, 10})
    assert before[8 * PAGE:10 * PAGE] == after[8 * PAGE:10 * PAGE]
    old_page = before[10 * PAGE:11 * PAGE]
    count = u16(old_page, 12)
    old_low = u16(old_page, 14 + (count - 1) * 2)
    assert old_page[old_low:] == after[10 * PAGE + old_low:11 * PAGE]
    assert old_page[14:14 + count * 2] == after[10 * PAGE + 14:10 * PAGE + 14 + count * 2]
    assert before[3 * PAGE:4 * PAGE] == after[3 * PAGE:4 * PAGE]
    facts.update({'changed_pages': changed, 'before_user_reference': old_reference,
                  'after_user_reference': reference, 'retired_rows_exact': True,
                  'database_body_exact': True, 'maps_and_data_index_exact': True,
                  'dictionary_policy': 'unchanged; possibly unused keys, unqualified hypothesis'})
    return bytes(after), facts
