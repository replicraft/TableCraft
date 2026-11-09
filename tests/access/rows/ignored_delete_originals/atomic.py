"""Independent atomic topology, map and unchanged-byte candidate contract."""

from support import wire

NODE_CHILD_WIDTH = 4
ROW_REFERENCE_WIDTH = 4
TABLE_COUNT_WIDTH = 4
RETIRED_PREFIX_WIDTH = 4
GLOBAL_MAP_REFERENCE = 256


def local_entries(raw, number):
    page = wire.page(raw, number)
    assert page[0] in [wire.LEAF_KIND, wire.NODE_KIND]
    ends = [byte * wire.BITS_PER_BYTE + bit
            for byte, mask in enumerate(page[wire.INDEX_MASK_START:wire.INDEX_BODY_START])
            for bit in range(wire.BITS_PER_BYTE) if mask & (1 << bit)]
    prefix = wire.u16(page, wire.INDEX_PREFIX_OFFSET)
    first, previous = None, 0
    result = []
    leaf = page[0] == wire.LEAF_KIND
    trailer = ROW_REFERENCE_WIDTH if leaf else ROW_REFERENCE_WIDTH + NODE_CHILD_WIDTH
    for end in ends:
        assert previous < end <= wire.INDEX_BODY_CAPACITY
        fragment = page[wire.INDEX_BODY_START + previous:wire.INDEX_BODY_START + end]
        full = fragment if first is None else first[:prefix] + fragment
        if first is None:
            first = full
        assert prefix <= len(first) and len(full) > trailer
        reference = wire.be32(full, len(full) - trailer)
        child = None if leaf else wire.be32(full, len(full) - NODE_CHILD_WIDTH)
        result.append((full[:-trailer].hex(), reference >> wire.REFERENCE_SLOT_BITS,
                       reference & wire.REFERENCE_SLOT_MASK, child))
        previous = end
    assert wire.u16(page, wire.INDEX_FREE_OFFSET) == wire.INDEX_BODY_CAPACITY - previous
    return result


def map_cell(raw, reference, target):
    number = reference >> wire.REFERENCE_SLOT_BITS
    slot = reference & wire.REFERENCE_SLOT_MASK
    page = wire.page(raw, number)
    start = wire.u16(page, wire.DATA_SLOT_START + slot * wire.SLOT_WIDTH) & wire.SLOT_START_MASK
    payload = wire.row(raw, reference)
    if payload[0] == wire.INLINE_MAP:
        relative = target - wire.u32(payload, wire.MAP_BODY_OFFSET)
        offset = wire.INLINE_BITS_OFFSET + relative // wire.BITS_PER_BYTE
        assert relative >= 0 and offset < len(payload)
        absolute = number * wire.PAGE_SIZE + start + offset
    else:
        assert payload[0] == wire.INDIRECT_MAP
        block, relative = divmod(target, wire.BITMAP_BITS)
        pointer_offset = wire.MAP_BODY_OFFSET + block * wire.POINTER_WIDTH
        assert pointer_offset + wire.POINTER_WIDTH <= len(payload)
        pointer = wire.u32(payload, pointer_offset)
        assert pointer and wire.page(raw, pointer)[:wire.BITMAP_BODY_OFFSET] == wire.BITMAP_SIGNATURE
        absolute = (pointer * wire.PAGE_SIZE + wire.BITMAP_BODY_OFFSET
                    + relative // wire.BITS_PER_BYTE)
    return absolute, 1 << (relative % wire.BITS_PER_BYTE)


def check(before, after, original, actual, selected_ids):
    assert len(before) == len(after), 'candidate file growth is forbidden'
    permitted = bytearray(len(before))

    def allow(offset, length):
        assert length >= 0 and 0 <= offset <= len(before) - length
        permitted[offset:offset + length] = bytes([1]) * length

    definition = original['definition']
    counter = definition * wire.PAGE_SIZE + wire.TABLE_ROW_COUNT_OFFSET
    assert wire.u32(after, counter) == original['count'] - len(selected_ids)
    allow(counter, TABLE_COUNT_WIDTH)
    selected_refs = set()
    for identifier in selected_ids:
        row = original['rows'][str(identifier)]
        page, slot = row['page'], row['slot']
        offset = page * wire.PAGE_SIZE + wire.DATA_SLOT_START + slot * wire.SLOT_WIDTH
        assert wire.u16(after, offset) == wire.u16(before, offset) | wire.TOMBSTONE_MASK
        selected_refs.add((page, slot))
        allow(offset, wire.SLOT_WIDTH)

    modified_maps, handled, all_retired, summaries = {}, set(), set(), []
    for name, old in original['indexes'].items():
        current = actual['indexes'][name]
        if old['physical'] in handled:
            continue
        handled.add(old['physical'])
        usage, root = wire.physical(before, definition, old['physical'])
        assert (usage, root) == wire.physical(after, definition, old['physical'])
        old_keys, old_leaves, old_nodes = wire.decode_tree(before, root, definition)
        new_keys, new_leaves, new_nodes = wire.decode_tree(after, root, definition)
        expected_keys = [entry for entry in old_keys if (entry[1], entry[2]) not in selected_refs]
        assert new_keys == expected_keys
        effective = {(entry[1], entry[2]) for entry in old_keys}.intersection(selected_refs)
        old_owned = set(wire.members(before, usage))
        new_owned = set(wire.members(after, usage))
        assert old_owned == {node['page'] for node in old_nodes + old_leaves}
        assert new_owned == {node['page'] for node in new_nodes + new_leaves}
        assert new_owned.issubset(old_owned) and root in new_owned
        retired = old_owned - new_owned
        assert not all_retired.intersection(retired)
        all_retired.update(retired)
        assert len(old_keys) - len(new_keys) == len(effective)

        if not effective:
            assert old_owned == new_owned
            assert all(wire.page(before, number) == wire.page(after, number) for number in old_owned)
            assert wire.row(before, usage) == wire.row(after, usage)
        else:
            # Original leaf partitions remain intact; only empty nonroot leaves retire.
            for leaf in old_leaves:
                number = leaf['page']
                original_entries = local_entries(before, number)
                expected = [entry for entry in original_entries
                            if (entry[1], entry[2]) not in selected_refs]
                if expected or number == root:
                    assert number in new_owned
                    assert local_entries(after, number) == expected
                else:
                    assert number in retired

            for node in old_nodes:
                number = node['page']
                if number in retired:
                    continue
                old_page, new_page = wire.page(before, number), wire.page(after, number)
                if not new_keys:
                    assert number == root and new_page[0] == wire.LEAF_KIND
                    continue
                assert new_page[0] == wire.NODE_KIND
                assert new_page[wire.INDEX_HEIGHT_OFFSET] == old_page[wire.INDEX_HEIGHT_OFFSET]
                children = [child for child in node['children'] if child in new_owned]
                assert children
                tail_form = bool(node['tail'])
                expected_tail = children[-1] if tail_form else 0
                assert wire.u32(new_page, wire.INDEX_TAIL_OFFSET) == expected_tail
                bounded = children[:-1] if tail_form else children
                expected_bounds = []
                for child in bounded:
                    child_keys, _, _ = wire.decode_tree(after, child, definition)
                    assert child_keys
                    maximum = child_keys[-1]
                    expected_bounds.append((*maximum, child))
                assert local_entries(after, number) == expected_bounds

            for number in retired:
                start = number * wire.PAGE_SIZE
                old_page, new_page = wire.page(before, number), wire.page(after, number)
                assert new_page[:RETIRED_PREFIX_WIDTH] == bytes(RETIRED_PREFIX_WIDTH)
                assert new_page[RETIRED_PREFIX_WIDTH:] == old_page[RETIRED_PREFIX_WIDTH:]
                allow(start, RETIRED_PREFIX_WIDTH)
                for reference, setting in [(usage, False), (GLOBAL_MAP_REFERENCE, True)]:
                    offset, mask = map_cell(before, reference, number)
                    assert bool(before[offset] & mask) != setting
                    previous = modified_maps.setdefault(offset, before[offset])
                    modified_maps[offset] = previous | mask if setting else previous & ~mask

            for number in new_owned:
                start = number * wire.PAGE_SIZE
                old_page, new_page = wire.page(before, number), wire.page(after, number)
                assert new_page[wire.PAGE_VERSION_OFFSET] == old_page[wire.PAGE_VERSION_OFFSET]
                owner_span = slice(wire.PAGE_OWNER_OFFSET, wire.INDEX_PREVIOUS_OFFSET)
                assert new_page[owner_span] == old_page[owner_span]
                if new_keys:
                    assert new_page[0] == old_page[0]
                    assert new_page[wire.INDEX_HEIGHT_OFFSET] == old_page[wire.INDEX_HEIGHT_OFFSET]
                else:
                    assert number == root and new_page[0] == wire.LEAF_KIND
                    assert new_page[wire.INDEX_HEIGHT_OFFSET] == 0
                    assert new_page[wire.INDEX_PREVIOUS_OFFSET:wire.INDEX_PREFIX_OFFSET] == bytes(
                        wire.INDEX_PREFIX_OFFSET - wire.INDEX_PREVIOUS_OFFSET)
                old_end = wire.PAGE_SIZE - wire.u16(old_page, wire.INDEX_FREE_OFFSET)
                new_end = wire.PAGE_SIZE - wire.u16(new_page, wire.INDEX_FREE_OFFSET)
                assert old_page[max(old_end, new_end):] == new_page[max(old_end, new_end):]
                allow(start, wire.PAGE_KIND_WIDTH)
                allow(start + wire.INDEX_FREE_OFFSET, wire.SLOT_WIDTH)
                allow(start + wire.INDEX_PREVIOUS_OFFSET,
                      max(old_end, new_end) - wire.INDEX_PREVIOUS_OFFSET)

        summaries.append({
            'physical': old['physical'], 'root': root,
            'eligible_before': len(old_keys), 'effective_deleted': len(effective),
            'eligible_after': len(new_keys), 'retired': sorted(retired),
            'effective_empty_pages_unchanged': not effective,
            'empty_index_with_live_table_rows': not new_keys and bool(actual['count']),
            'height_before': before[root * wire.PAGE_SIZE + wire.INDEX_HEIGHT_OFFSET],
            'height_after': after[root * wire.PAGE_SIZE + wire.INDEX_HEIGHT_OFFSET],
        })

    for offset, expected in modified_maps.items():
        assert after[offset] == expected
        allow(offset, 1)
    for offset, (old, new) in enumerate(zip(before, after)):
        assert permitted[offset] or old == new, ('unpermitted changed byte', offset)
    return {'retired_pages': sorted(all_retired), 'distinct_physical_indexes': summaries,
            'original_leaf_partitions_preserved': True, 'original_nonempty_heights_preserved': True,
            'retired_original_residuals_preserved': True, 'file_length_unchanged': True,
            'only_exact_map_bits_changed': True, 'all_other_bytes_unchanged': True}
