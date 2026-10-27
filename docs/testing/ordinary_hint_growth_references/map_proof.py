"""Independent role membership, inline-window replay and bitmap byte proofs."""
PAGE = 4096
ROW_BYTES = 69
BITMAP_PREFIX = bytes([5, 1, 0, 0])
BITMAP_CAPACITY = (PAGE - len(BITMAP_PREFIX)) * 8


def extent(metadata):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    end = metadata['table_page'] * PAGE + metadata['row_end']
    assert end - start == ROW_BYTES
    return start, end


def inline(base, members):
    row = bytearray(ROW_BYTES)
    row[1:5] = base.to_bytes(4, 'little')
    for member in members:
        relative = member - base
        assert 0 <= relative < 512
        row[5 + relative // 8] |= 1 << (relative % 8)
    return bytes(row)


def replay(original_row, original_members, removed, added):
    members = set(original_members) - set(removed)
    assert set(removed) <= set(original_members)
    kind = original_row[0]
    assert kind in (0, 1)
    base = int.from_bytes(original_row[1:5], 'little') if kind == 0 else None
    events = []
    for member in sorted(added):
        assert member not in members
        transition = 'indirect-represented' if kind == 1 else 'represented'
        if kind == 0 and not base <= member < base + 512:
            minimum = min(members | {member})
            maximum = max(members | {member})
            aligned = minimum - minimum % 8
            if maximum - aligned + 1 < 512:
                base = aligned
                transition = 'inline-rebase'
            else:
                kind = 1
                base = None
                transition = 'indirect-promotion'
        members.add(member)
        events.append({'member': member, 'transition': transition, 'base_after': base})
    return kind, base, members, events


def compare(old, new, before, after, removed, added, allowed):
    assert set(before['members']) - set(after['members']) == set(removed)
    assert set(after['members']) - set(before['members']) == set(added)
    for field in ('table_page', 'row_start', 'row_end'):
        assert before[field] == after[field]
    start, end = extent(before)
    assert extent(after) == (start, end)
    old_row = old[start:end]
    new_row = new[start:end]
    kind, base, members, events = replay(old_row, before['members'], removed, added)
    assert members == set(after['members'])
    assert new_row[0] == kind
    metadata = set()
    changed_bitmap_bytes = 0
    if kind == 0:
        expected = inline(base, members)
        assert new_row == expected
        assert not after['decoded']['bitmap_pages']
    else:
        pointers = [int.from_bytes(new_row[1 + position * 4:5 + position * 4], 'little') for position in range(16)]
        # The bounded originals and outputs fit the first 32,736-page bitmap.
        assert pointers[0] != 0 and not any(pointers[1:])
        bitmap = pointers[0]
        assert after['decoded']['bitmap_pages'] == [bitmap]
        expected = bytes([1]) + bitmap.to_bytes(4, 'little') + bytes(64)
        assert new_row == expected
        if old_row[0] == 1:
            assert new_row == old_row
            assert before['decoded']['bitmap_pages'] == [bitmap]
            header = old[bitmap * PAGE:bitmap * PAGE + 4]
            assert header == BITMAP_PREFIX
        else:
            assert bitmap >= len(old) // PAGE
            header = BITMAP_PREFIX
            metadata.add(bitmap)
        block = bytearray(PAGE)
        block[:4] = header
        for member in members:
            assert 0 <= member < BITMAP_CAPACITY
            block[4 + member // 8] |= 1 << (member % 8)
        actual = new[bitmap * PAGE:(bitmap + 1) * PAGE]
        assert actual == block
        if bitmap < len(old) // PAGE:
            for position, (left, right) in enumerate(zip(old[bitmap * PAGE:(bitmap + 1) * PAGE], block, strict=True)):
                if left != right:
                    allowed[bitmap * PAGE + position] = 1
                    changed_bitmap_bytes += 1
    allowed[start:end] = bytes([1]) * ROW_BYTES
    return {
        'events': events,
        'kind_before': old_row[0],
        'kind_after': kind,
        'base_before': int.from_bytes(old_row[1:5], 'little') if old_row[0] == 0 else None,
        'base_after': base,
        'fresh_bitmap_pages': sorted(metadata),
        'old_bitmap_changed_bytes': changed_bitmap_bytes,
    }
