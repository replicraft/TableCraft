"""Independent literal byte authorization for the fixed eight original controls."""
import hashlib

PAGE_BYTES = 4096
SLOT_START = 14
SLOT_BYTES = 2
FREE_OFFSET = 2
COUNT_OFFSET = 12
OWNER_OFFSET = 4
HIDDEN = 0x8000
FORWARDED = 0x4000
OFFSET_MASK = 0x1fff
POINTER_BYTES = 4


def word(raw, offset):
    return int.from_bytes(raw[offset:offset + SLOT_BYTES], 'little')


def frame(raw, page, slot):
    base = page * PAGE_BYTES
    at = base + SLOT_START + SLOT_BYTES * slot
    entry = word(raw, at)
    start = base + (entry & OFFSET_MASK)
    end = base + (word(raw, at - SLOT_BYTES) & OFFSET_MASK) if slot else base + PAGE_BYTES
    return at, entry, start, end


def same_semantics(actual, reference):
    assert actual.keys() == reference.keys()
    for name, table in actual.items():
        expected = reference[name]
        assert table['indexes'] == expected['indexes'], (name, 'index metadata/order')
        assert table['rows'].keys() == expected['rows'].keys()
        for identity, row in table['rows'].items():
            assert row['values'] == expected['rows'][identity]['values'], (name, identity)


def prove(case, before, after, state_before, state_after, metadata):
    assert len(after) == len(before) + case['expected_file_growth_bytes']
    root_page = case['root_page']
    target_page = case['target_page']
    target_slot = case['target_slot']
    root_at, root_word, root_start, root_end = frame(before, root_page, 0)
    _, new_word, new_start, new_end = frame(after, target_page, target_slot)
    assert new_word & ~OFFSET_MASK == HIDDEN
    assert new_end - new_start == case['target_capacity_bytes'] == 268
    assert state_before['declared_rows'] == state_after['declared_rows'] == case['table_and_first_index_historical_count']
    assert state_before['historical_index_rows'] == state_after['historical_index_rows'] == case['table_and_first_index_historical_count']
    expected = bytearray(before)
    expected.extend(bytes(case['expected_file_growth_bytes']))
    if case['action'] == 'grow':
        original = before[root_start:root_end]
        assert len(original) == 13 and original[:6] == bytes.fromhex('020001000000')
        payload = bytes.fromhex(case['replacement_hex'])
        end = 6 + len(payload)
        body = original[:6] + payload + end.to_bytes(2, 'little') + bytes.fromhex('06000100') + bytes([original[-1] | 2])
        assert len(body) == 268
        expected[root_start:root_end] = ((target_page << 8) | target_slot).to_bytes(4, 'little') + bytes(root_end - root_start - POINTER_BYTES)
        expected[root_at:root_at + SLOT_BYTES] = (root_word | FORWARDED).to_bytes(2, 'little')
        base = target_page * PAGE_BYTES
        if target_page * PAGE_BYTES == len(before):
            literal = bytearray(PAGE_BYTES)
            literal[:2] = bytes([1, 1])
            definition = metadata[case['table']]['definition']
            literal[OWNER_OFFSET:OWNER_OFFSET + 4] = definition.to_bytes(4, 'little')
            count = 0
            old_end = PAGE_BYTES
            expected[base:base + PAGE_BYTES] = literal
        else:
            count = word(before, base + COUNT_OFFSET)
            assert count == target_slot
            old_end = frame(before, target_page, count - 1)[2] - base if count else PAGE_BYTES
        start = old_end - len(body)
        assert new_start == base + start and new_end == base + old_end
        expected[base + start:base + old_end] = body
        at = base + SLOT_START + SLOT_BYTES * count
        expected[at:at + SLOT_BYTES] = (start | HIDDEN).to_bytes(2, 'little')
        expected[base + COUNT_OFFSET:base + COUNT_OFFSET + SLOT_BYTES] = (count + 1).to_bytes(2, 'little')
        free = start - SLOT_START - SLOT_BYTES * (count + 1)
        expected[base + FREE_OFFSET:base + FREE_OFFSET + SLOT_BYTES] = free.to_bytes(2, 'little')
    else:
        _, old_word, old_start, old_end = frame(before, target_page, target_slot)
        assert (old_word, old_start, old_end) == (new_word, new_start, new_end)
        assert root_word & FORWARDED
        assert before[root_start:root_start + POINTER_BYTES] == ((target_page << 8) | target_slot).to_bytes(4, 'little')
        # Preserve allocated capacity and the old Binary bytes beyond the new span.
        payload = bytes.fromhex(case['replacement_hex'])
        expected[old_start + 6:old_start + 6 + len(payload)] = payload
        expected[old_end - 7:old_end - 5] = (6 + len(payload)).to_bytes(2, 'little')
    actual_changes = []
    for role, changes in case['expected_maps_from_public_raw'].items():
        for change in changes['changed_existing_bytes']:
            offset = change['offset']
            assert before[offset] == change['before'], (role, offset)
            assert expected[offset] == change['before'], 'map overlaps another authorized write'
            expected[offset] = change['after']
            actual_changes.append({'role': role, **change})
    assert bytes(expected) == after, 'whole-image independent expected-byte proof'
    indexes = []
    for number in range(len(before) // PAGE_BYTES):
        base = number * PAGE_BYTES
        if before[base:base + 2] in (b'\x03\x01', b'\x04\x01'):
            assert before[base:base + PAGE_BYTES] == after[base:base + PAGE_BYTES]
            indexes.append(number)
    return {'whole_image_allowed_bytes_exact': True, 'full_new_page_literal_exact': bool(case['expected_file_growth_bytes']),
            'all_preexisting_raw_index_pages_unchanged': indexes, 'map_byte_changes': actual_changes,
            'image_sha256': hashlib.sha256(after).hexdigest(), 'target_capacity': 268,
            'candidate_padding_contract_distinct_from_reference': case['action'] == 'shrink'}
