"""Independent map decoding for original read-only DELETE reference captures."""

MAP_INLINE = 0
MAP_INDIRECT = 1
PAGE_POINTER_BYTES = 4
INLINE_BASE_OFFSET = 1
INLINE_BITMAP_OFFSET = INLINE_BASE_OFFSET + PAGE_POINTER_BYTES
INDIRECT_BITMAP_OFFSET = 4
USAGE_PAGE_ROW_COUNT_OFFSET = 12
ROW_DIRECTORY_OFFSET = 14
ROW_SLOT_BYTES = 2
ROW_PAYLOAD_OFFSET_MASK = 0x1FFF
MASK_BITS = 8
USAGE_BITMAP_SIGNATURE = b'\x05\x01'


def unsigned(data, offset, width=PAGE_POINTER_BYTES):
    return int.from_bytes(data[offset:offset + width], 'little')


def parse(text):
    result = {'maps': {}, 'indexes': {}}
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'FILE':
            result['page_bytes'], result['page_count'] = map(int, fields[1:])
        elif tag == 'FORMAT':
            result['format_offsets'] = list(map(int, fields[1:]))
        elif tag == 'MAP':
            names = (
                'table_page', 'row_start', 'row_end', 'start_page', 'end_page',
                'reported_count',
            )
            metadata = dict(zip(names, map(int, fields[2:]), strict=True))
            metadata['file_members'] = []
            if fields[1] in result['maps']:
                raise AssertionError('Duplicate map metadata')
            result['maps'][fields[1]] = metadata
        elif tag == 'MEMBER':
            result['maps'][fields[1]]['file_members'].append(int(fields[2]))
        elif tag == 'INDEX':
            names = ('physical', 'root', 'owned_count', 'entry_count')
            if fields[1] in result['indexes']:
                raise AssertionError('Duplicate index metadata')
            result['indexes'][fields[1]] = dict(
                zip(names, map(int, fields[2:]), strict=True),
            )
        else:
            raise AssertionError('Unexpected pinned probe record')
    return result


def decode_map(data, metadata, width, file_pages):
    page_start = metadata['table_page'] * width
    start = page_start + metadata['row_start']
    end = page_start + metadata['row_end']
    if not page_start <= start < end <= page_start + width:
        raise AssertionError('Map extent exceeds its page')
    row = data[start:end]
    matches = []
    # Independently match the protected metadata to the literal usage-page slot
    # words. These special map payloads do not have ordinary table row headers.
    rows = unsigned(data, page_start + USAGE_PAGE_ROW_COUNT_OFFSET, ROW_SLOT_BYTES)
    for slot in range(rows):
        word = unsigned(data, page_start + ROW_DIRECTORY_OFFSET + slot * ROW_SLOT_BYTES, ROW_SLOT_BYTES)
        row_start = page_start + (word & ROW_PAYLOAD_OFFSET_MASK)
        if slot == 0:
            row_end = page_start + width
        else:
            preceding = unsigned(data, page_start + ROW_DIRECTORY_OFFSET + (slot - 1) * ROW_SLOT_BYTES, ROW_SLOT_BYTES)
            row_end = page_start + (preceding & ROW_PAYLOAD_OFFSET_MASK)
        if (row_start, row_end) == (start, end):
            matches.append(slot)
    if len(matches) != 1:
        raise AssertionError('Map geometry does not match exactly one raw slot')

    ranges = []
    bitmap_pages = []
    if row[0] == MAP_INLINE:
        ranges.append((unsigned(row, INLINE_BASE_OFFSET), start + INLINE_BITMAP_OFFSET, row[INLINE_BITMAP_OFFSET:]))
    elif row[0] == MAP_INDIRECT:
        if (len(row) - INLINE_BASE_OFFSET) % PAGE_POINTER_BYTES:
            raise AssertionError('Indirect map row width is invalid')
        for slot in range((len(row) - INLINE_BASE_OFFSET) // PAGE_POINTER_BYTES):
            pointer = unsigned(row, INLINE_BASE_OFFSET + slot * PAGE_POINTER_BYTES)
            if pointer == 0:
                continue
            if pointer >= file_pages:
                raise AssertionError('Indirect bitmap is outside the file')
            bitmap = data[pointer * width:(pointer + 1) * width]
            if bitmap[:len(USAGE_BITMAP_SIGNATURE)] != USAGE_BITMAP_SIGNATURE:
                raise AssertionError('Indirect bitmap signature differs')
            ranges.append((
                slot * (width - INDIRECT_BITMAP_OFFSET) * MASK_BITS,
                pointer * width + INDIRECT_BITMAP_OFFSET,
                bitmap[INDIRECT_BITMAP_OFFSET:],
            ))
            bitmap_pages.append(pointer)
    else:
        raise AssertionError('Unobserved map payload type')

    all_members = []
    locations = {}
    for base, physical, bitmap in ranges:
        for byte_index, value in enumerate(bitmap):
            for bit in range(MASK_BITS):
                page = base + byte_index * MASK_BITS + bit
                if page < file_pages:
                    locations[page] = {
                        'offset': physical + byte_index,
                        'mask': 1 << bit,
                    }
                if value & (1 << bit):
                    all_members.append(page)
    bounded = []
    for page in all_members:
        if page < file_pages:
            bounded.append(page)
    if bounded != metadata['file_members']:
        raise AssertionError('Raw bitmap disagrees with getter/BitSet members')
    if len(all_members) != metadata['reported_count']:
        raise AssertionError('Raw complete map count disagrees with getter')
    return {
        'payload_type': row[0],
        'map_page': metadata['table_page'],
        'map_slot': matches[0],
        'row_start': start,
        'row_end': end,
        'bitmap_pages': bitmap_pages,
        'complete_set_bit_count': len(all_members),
        'file_members': bounded,
        'outside_file_set_bit_count': len(all_members) - len(bounded),
        'bit_locations': locations,
    }


def verify_transition(before, after, before_maps, after_maps, deltas, width):
    if set(before_maps) != set(after_maps) or set(deltas) != set(before_maps):
        raise AssertionError('Map inventory changed')
    retired = set()
    spans_checked = 0
    for name, original in before_maps.items():
        candidate = after_maps[name]
        for field in ('payload_type', 'map_page', 'map_slot', 'row_start', 'row_end'):
            if candidate[field] != original[field]:
                raise AssertionError('Existing map representation/extent changed')
        delta = deltas[name]
        removed = delta['removed_members']
        added = delta['added_members']
        if name.startswith('Index'):
            retired.update(removed)
            if added:
                raise AssertionError('Reference added an index-owned page')
        elif name != 'JackcessGlobalFree' and (removed or added):
            raise AssertionError('Table owned/free-space membership changed')
        row_start = original['row_start']
        row_end = original['row_end']
        row = before[row_start:row_end]
        if row[0] == MAP_INLINE:
            spans = [(row_start, row_end)]
        else:
            if before[row_start:row_end] != after[row_start:row_end]:
                raise AssertionError('Existing indirect pointer row changed')
            spans = []
            for offset in range(INLINE_BASE_OFFSET, len(row), PAGE_POINTER_BYTES):
                page = unsigned(row, offset)
                if page:
                    spans.append((page * width, (page + 1) * width))
        for start, end in spans:
            expected = bytearray(before[start:end])
            for page in removed + added:
                location = original['bit_locations'][page]
                if location != candidate['bit_locations'][page]:
                    raise AssertionError('Physical bitmap bit address changed')
                offset = location['offset']
                if start <= offset < end:
                    if page in added:
                        expected[offset - start] |= location['mask']
                    else:
                        expected[offset - start] &= ~location['mask']
            if bytes(expected) != after[start:end]:
                raise AssertionError('Map bytes changed beyond exact membership-bit deltas')
            spans_checked += 1
    global_delta = deltas['JackcessGlobalFree']
    if global_delta['removed_members'] or set(global_delta['added_members']) != retired:
        raise AssertionError('Global in-file additions differ from index-owned retirements')
    return {
        'map_count': len(before_maps),
        'complete_payload_or_bitmap_spans_checked': spans_checked,
        'map_representation_and_pointer_rows_unchanged': True,
        'only_exact_membership_bits_changed': True,
        'global_in_file_additions_equal_retired_index_pages': True,
        'retired_index_pages': sorted(retired),
    }
