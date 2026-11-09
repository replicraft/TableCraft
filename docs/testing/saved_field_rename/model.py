"""Independent same-width raw field rename hypothesis, not a native writer.

Literal TD/MR2 framing follows maintained first-party ADD byte models. Only
pinned generated sources with actual explicit OFF enter this experiment.
"""
import struct

PAGE = 4096
CATALOG = 4
DEFINITION = 7
PROPERTY = 10
OFFSET_MASK = 0x1FFF


def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]


def u32(data, at):
    return struct.unpack_from('<I', data, at)[0]


def row_span(image, page, slot):
    base = page * PAGE
    count = u16(image, base + 12)
    assert 0 <= slot < count
    start = u16(image, base + 14 + slot * 2) & OFFSET_MASK
    end = PAGE if slot == 0 else u16(image, base + 14 + (slot - 1) * 2) & OFFSET_MASK
    assert 14 + count * 2 <= start < end <= PAGE
    return base + start, base + end


def live_user_body(image):
    root_start, _ = row_span(image, CATALOG, 7)
    word = u16(image, CATALOG * PAGE + 14 + 7 * 2)
    slot = 7
    if word & 0x4000:
        reference = u32(image, root_start)
        assert reference >> 8 == CATALOG
        slot = reference & 255
        assert u16(image, CATALOG * PAGE + 14 + slot * 2) & 0x8000
    start, end = row_span(image, CATALOG, slot)
    record = image[start:end]
    footer = len(record) - 5
    begin = u16(record, footer - 18)
    finish = u16(record, footer - 20)
    assert finish - begin == 12
    tagged = u32(record, begin)
    reference = u32(record, begin + 4)
    assert tagged & 0xC0000000 == 0x40000000
    assert u32(record, begin + 8) == 0
    assert reference >> 8 == PROPERTY
    body_start, body_end = row_span(image, PROPERTY, reference & 255)
    assert body_end - body_start == tagged & 0x3FFFFFFF
    # OFF is its own last row, not another user ADD history snapshot.
    assert (reference & 255) == u16(image, PROPERTY * PAGE + 12) - 2
    return body_start, body_end, slot, reference


def field_name(image, old):
    base = DEFINITION * PAGE
    count = u16(image, base + 41)
    assert count == u16(image, base + 45)
    names = base + 75 + count * 25
    found = []
    for index in range(count):
        length = u16(image, names)
        begin = names + 2
        end = begin + length
        name = image[begin:end].decode('utf-16le')
        if name == old:
            found.append((index, begin, end))
        names = end
    assert len(found) == 1
    index, begin, end = found[0]
    assert index != 0
    descriptor = image[base + 75 + index * 25:base + 75 + (index + 1) * 25]
    return begin, end, {
        'index': index, 'native_number': u16(descriptor, 5),
        'variable_ordinal': u16(descriptor, 7), 'column_id': u16(descriptor, 9),
        'type': descriptor[0], 'flags': descriptor[15],
        'extended_flags': descriptor[16], 'width_bytes': u16(descriptor, 23),
        'descriptor_hex': descriptor.hex(),
    }


def property_group(image, begin, end, old):
    body = image[begin:end]
    assert body[:4] == b'MR2\0'
    dictionary_end = 4 + u32(body, 4)
    position = dictionary_end
    found = []
    while position < len(body):
        length = u32(body, position)
        kind = u16(body, position + 4)
        name_record_length = u32(body, position + 6)
        name_bytes = u16(body, position + 10)
        assert name_record_length == name_bytes + 6
        name_start = position + 12
        name_end = name_start + name_bytes
        assert name_end <= position + length <= len(body)
        name = body[name_start:name_end].decode('utf-16le')
        if kind == 1 and name == old:
            found.append((begin + name_start, begin + name_end))
        position += length
    assert position == len(body) and len(found) == 1
    return found[0]


def rename(image, old, new, rename_live_group):
    old_bytes = old.encode('utf-16le')
    new_bytes = new.encode('utf-16le')
    assert old.isascii() and new.isascii() and len(old_bytes) == len(new_bytes)
    assert len(image) == 11 * PAGE
    name_start, name_end, facts = field_name(image, old)
    body_start, body_end, physical, reference = live_user_body(image)
    group_start, group_end = property_group(image, body_start, body_end, old)
    assert image[name_start:name_end] == image[group_start:group_end] == old_bytes
    result = bytearray(image)
    result[name_start:name_end] = new_bytes
    scope = [[name_start, name_end]]
    if rename_live_group:
        result[group_start:group_end] = new_bytes
        scope.append([group_start, group_end])
    assert all(a == b or any(start <= at < end for start, end in scope)
               for at, (a, b) in enumerate(zip(image, result)))
    facts.update({'catalog_physical_slot': physical, 'live_user_reference': reference,
                  'live_body_span': [body_start, body_end], 'changed_spans': scope,
                  'live_group_name_span': [group_start, group_end],
                  'changed_pages': [page for page in range(11)
                                    if image[page * PAGE:(page + 1) * PAGE] != result[page * PAGE:(page + 1) * PAGE]],
                  'maps_and_catalog_unchanged': image[2 * PAGE:5 * PAGE] == result[2 * PAGE:5 * PAGE],
                  'all_old_history_rows_unchanged': image[body_end:11 * PAGE] == result[body_end:11 * PAGE],
                  'new_db_role_body_unchanged': image[10 * PAGE:body_start] == result[10 * PAGE:body_start],
                  'user_data_index_pages_unchanged': image[8 * PAGE:10 * PAGE] == result[8 * PAGE:10 * PAGE]})
    return bytes(result), facts
