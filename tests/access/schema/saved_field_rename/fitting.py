"""First-party fitted ASCII rename images, independent of native writer code.

The retained source slots remain fixed. Earlier property histories stay at their
original addresses; the live user row keeps its end and the following OFF body
moves by the opposite name-size delta. Newly freed bytes retain their old values.
"""
import struct

from model import (
    CATALOG, DEFINITION, OFFSET_MASK, PAGE, PROPERTY,
    field_name, live_user_body, property_group, row_span, u16, u32,
)

DIRECTORY = 14
FREE = 2
DEFINITION_LENGTH = 8
GROUP_NAME_PREFIX = 12
SINGLE = 0x40000000
LENGTH_MASK = 0x3FFFFFFF


def put16(data, at, value):
    struct.pack_into('<H', data, at, value)


def put32(data, at, value):
    struct.pack_into('<I', data, at, value)


def descriptor_offset(image, physical_slot):
    start, end = row_span(image, CATALOG, physical_slot)
    record = image[start:end]
    begin = u16(record, len(record) - 5 - 18)
    finish = u16(record, len(record) - 5 - 20)
    assert finish - begin == 12
    return start + begin


def rename(image, old, new, variant='coordinated'):
    assert old.isascii() and new.isascii() and 1 <= len(new) <= 64
    assert len(old) != len(new)
    assert len(image) == 11 * PAGE
    old_bytes = old.encode('utf-16le')
    new_bytes = new.encode('utf-16le')
    delta = len(new_bytes) - len(old_bytes)
    name_start, name_end, facts = field_name(image, old)
    body_start, body_end, physical, reference = live_user_body(image)
    group_start, group_end = property_group(image, body_start, body_end, old)
    assert image[name_start:name_end] == image[group_start:group_end] == old_bytes
    descriptor = descriptor_offset(image, physical)
    assert u32(image, descriptor) == SINGLE | (body_end - body_start)

    result = bytearray(image)
    definition_base = DEFINITION * PAGE
    definition_length = u32(image, definition_base + DEFINITION_LENGTH)
    definition_end = definition_base + definition_length
    assert name_end <= definition_end <= (DEFINITION + 1) * PAGE
    assert 0 <= definition_length + delta <= PAGE - 8
    assert u16(image, definition_base + FREE) == PAGE - definition_length - 8
    definition_tail = image[name_end:definition_end]
    result[name_start:name_start + len(new_bytes)] = new_bytes
    result[name_start + len(new_bytes):definition_end + delta] = definition_tail
    put16(result, name_start - 2, len(new_bytes))
    put32(result, definition_base + DEFINITION_LENGTH, definition_length + delta)
    put16(result, definition_base + FREE, u16(image, definition_base + FREE) - delta)
    if variant == 'tdef-only':
        return bytes(result), {'delta_bytes': delta, 'variant': variant}

    group = group_start - GROUP_NAME_PREFIX
    relative_group = group - body_start
    relative_name = group_start - body_start
    relative_end = group_end - body_start
    original_body = image[body_start:body_end]
    body = bytearray(original_body[:relative_name] + new_bytes + original_body[relative_end:])
    put32(body, relative_group, u32(original_body, relative_group) + delta)
    put32(body, relative_group + 6, u32(original_body, relative_group + 6) + delta)
    put16(body, relative_group + 10, len(new_bytes))

    property_base = PROPERTY * PAGE
    rows = u16(image, property_base + 12)
    user_slot = reference & 255
    assert user_slot == rows - 2
    db_start, db_end = row_span(image, PROPERTY, rows - 1)
    assert db_end == body_start
    assert u16(image, property_base + FREE) == db_start - property_base - DIRECTORY - 2 * rows
    assert db_start - delta >= property_base + DIRECTORY + 2 * rows
    db_body = image[db_start:db_end]
    result[db_start - delta:db_end - delta] = db_body
    result[body_start - delta:body_end] = body
    put16(result, property_base + DIRECTORY + 2 * user_slot, body_start - delta - property_base)
    put16(result, property_base + DIRECTORY + 2 * (rows - 1), db_start - delta - property_base)
    put16(result, property_base + FREE, u16(image, property_base + FREE) - delta)
    put32(result, descriptor, SINGLE | len(body))

    if variant == 'stale-descriptor':
        put32(result, descriptor, u32(image, descriptor))
    elif variant == 'stale-user-row':
        at = property_base + DIRECTORY + 2 * user_slot
        put16(result, at, u16(image, at))
    else:
        assert variant == 'coordinated'
    assert len(result) == len(image)
    facts.update({
        'variant': variant,
        'delta_bytes': delta,
        'catalog_physical_slot': physical,
        'live_user_reference': reference,
        'descriptor_length_offset': descriptor,
        'before_live_body_span': [body_start, body_end],
        'after_live_body_span': [body_start - delta, body_end],
        'before_db_body_span': [db_start, db_end],
        'after_db_body_span': [db_start - delta, db_end - delta],
        'before_tdef_length': definition_length,
        'after_tdef_length': definition_length + delta,
        'changed_pages': [page for page in range(11)
                          if image[page * PAGE:(page + 1) * PAGE]
                          != result[page * PAGE:(page + 1) * PAGE]],
        'retired_history_rows_unchanged': image[body_end:(PROPERTY + 1) * PAGE]
        == result[body_end:(PROPERTY + 1) * PAGE],
        'db_off_payload_unchanged': db_body == result[db_start - delta:db_end - delta],
        'data_index_and_maps_unchanged': image[2 * PAGE:4 * PAGE] == result[2 * PAGE:4 * PAGE]
        and image[8 * PAGE:10 * PAGE] == result[8 * PAGE:10 * PAGE],
    })
    return bytes(result), facts
