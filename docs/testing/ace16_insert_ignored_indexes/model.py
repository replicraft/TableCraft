"""Source/request engineering images; no public AFTER or candidate authority."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_multiple_indexes import model as frames
from ace16_insert_ignored_indexes import reference

PAGE = 4096
ENTRY_START = 480
CAPACITY = 3616
MAX_SLOTS = 255
MAX_IMAGE_BYTES = 8 * 1024 * 1024


def leaf(original, entries, seed, owner, previous=0, next_=0):
    page = frames.leaf(original, entries, seed, owner, previous, next_)
    if original is not None:
        # Retained serializer owns free/prefix, links, bitmap and active bytes.
        # Keep all other original header fields, as well as the old suffix.
        page[4:12] = original[4:12]
        page[20:24] = original[20:24]
        page[26] = original[26]
    return page


def grown_root(original, maximum, left, right, owner):
    page = frames.root(original, maximum, left, right, owner)
    page[4:20] = original[4:20]
    return page


def declared_requests(case, supplied):
    requested = reference.requests(case['variant'])
    if supplied is not None:
        assert list(supplied) == requested
    if 'requests' in case:
        assert case['requests'] == [[row[0], row[1]] for row in requested]
    assert requested and all(row[2] == b'' for row in requested)
    return requested


def append_records(raw, before, requested):
    definition = before['table']['definition']
    all_rows = dict(before['rows'])
    hints = set(before['maps']['hint']['members'])
    new_data = []
    placements = []
    for values in requested:
        body = reference.record(values)
        highest = max(hints) if hints else None
        destination = None
        if highest is not None:
            count = reference.n(raw, highest * PAGE + 12, 2)
            free = reference.n(raw, highest * PAGE + 2, 2)
            if count < MAX_SLOTS and free >= len(body) + 2:
                destination = highest
            else:
                hints.remove(highest)
        if destination is None:
            destination = len(raw) // PAGE
            raw.extend(bytes(PAGE))
            raw[destination * PAGE:destination * PAGE + 2] = bytes([1, 1])
            frames.put(raw, destination * PAGE + 4, definition)
            frames.put(raw, destination * PAGE + 2, PAGE - 14, 2)
            new_data.append(destination)
            hints.add(destination)

        base = destination * PAGE
        assert raw[base:base + 2] == bytes([1, 1])
        assert reference.n(raw, base + 4) == definition
        slot = reference.n(raw, base + 12, 2)
        assert slot < MAX_SLOTS
        end = PAGE if slot == 0 else reference.n(raw, base + 12 + 2 * slot, 2)
        begin = end - len(body)
        assert begin >= 14 + 2 * (slot + 1)
        raw[base + begin:base + end] = body
        frames.put(raw, base + 14 + 2 * slot, begin, 2)
        frames.put(raw, base + 12, slot + 1, 2)
        frames.put(raw, base + 2, begin - 14 - 2 * (slot + 1), 2)
        logical = destination * 256 + slot
        assert logical not in all_rows
        all_rows[logical] = values
        placements.append({
            'id': values[0],
            'page': destination,
            'slot': slot,
            'ROW': logical,
            'record_hex': body.hex(),
        })
    return all_rows, hints, new_data, placements


def index_entries(rows, physical):
    return sorted(
        reference.key(values[physical], logical)
        for logical, values in rows.items()
        if physical == 0 or values[physical] is not None
    )


def maintain_indexes(raw, before, all_rows, placements):
    definition = before['table']['definition']
    final_members = []
    allocations = []
    unchanged = []
    assert len(before['physical']) == len(before['trees']) == 2
    for physical, metadata in enumerate(before['physical']):
        root = metadata['root']
        original = before['raw'][root * PAGE:(root + 1) * PAGE]
        old_members = set(before['trees'][physical])
        old_entries = index_entries(before['rows'], physical)
        _, _, captured = reference.tree(before['raw'], root, definition)
        assert captured == old_entries
        entries = index_entries(all_rows, physical)
        added = [
            item for item in placements
            if physical == 0 or all_rows[item['ROW']][physical] is not None
        ]
        assert len(entries) == len(old_entries) + len(added)
        if not added:
            assert entries == old_entries
            final_members.append(old_members)
            unchanged.append(physical)
            continue

        # The approved before-source profile requires changed owners to be
        # original root leaves. The secondary NODE occurs only in the no-op case.
        assert old_members == {root}
        assert original[0] == 4 and original[26] == 0
        assert reference.n(original, 12) == reference.n(original, 16) == reference.n(original, 20) == 0
        seed = reference.n(original, 24, 2)
        if frames.framing(entries, seed)[1] <= CAPACITY:
            raw[root * PAGE:(root + 1) * PAGE] = leaf(original, entries, seed, definition)
            members = old_members
        else:
            middle = len(entries) // 2
            assert 0 < middle < len(entries)
            assert frames.framing(entries[:middle], 0)[1] <= CAPACITY
            assert frames.framing(entries[middle:], 0)[1] <= CAPACITY
            right = len(raw) // PAGE
            left = right + 1
            raw.extend(bytes(2 * PAGE))
            raw[right * PAGE:(right + 1) * PAGE] = leaf(
                None, entries[middle:], 0, definition, left, 0,
            )
            raw[left * PAGE:(left + 1) * PAGE] = leaf(
                None, entries[:middle], 0, definition, 0, right,
            )
            raw[root * PAGE:(root + 1) * PAGE] = grown_root(
                original, entries[middle - 1], left, right, definition,
            )
            members = old_members | {left, right}
            allocations.extend([
                {'physical': physical, 'purpose': 'INDEX', 'page': right},
                {'physical': physical, 'purpose': 'INDEX', 'page': left},
            ])
        final_members.append(members)
    return final_members, allocations, unchanged


def derive(case, before, supplied_requests=None):
    """Return complete engineering bytes and JSON facts, without filesystem I/O."""
    requested = declared_requests(case, supplied_requests)
    raw = bytearray(before['raw'])
    assert 0 < len(raw) <= MAX_IMAGE_BYTES and len(raw) % PAGE == 0
    definition = before['table']['definition']
    all_rows, hints, new_data, placements = append_records(raw, before, requested)
    final_members, allocations, unchanged = maintain_indexes(raw, before, all_rows, placements)
    original_eof = len(before['raw']) // PAGE
    suffix = set(range(original_eof, len(raw) // PAGE))
    new_indexes = {item['page'] for item in allocations}
    assert suffix == set(new_data) | new_indexes
    assert not set(new_data) & new_indexes
    assert suffix <= before['maps']['global']['members']

    changes = {
        'owned': before['maps']['owned']['members'] | set(new_data),
        'hint': hints,
        'global': before['maps']['global']['members'] - suffix,
        'index0': final_members[0],
        'index1': final_members[1],
    }
    map_changes = []
    for role, wanted in changes.items():
        metadata = before['maps'][role]
        for member in sorted(metadata['members'] ^ wanted):
            # This finite recipe requires original represented future cells.
            # A source outside that contract refuses, with no geometry repair.
            assert member in metadata['locations']
            offset, mask = metadata['locations'][member]
            old = raw[offset]
            raw[offset] ^= mask
            map_changes.append({'role': role, 'member': member, 'offset': offset,
                                'mask': mask, 'before': old, 'after': raw[offset]})

    frames.put(raw, definition * PAGE + 16, len(all_rows))
    original_families = {values[1] for values in before['rows'].values() if values[1] is not None}
    added_families = {values[1] for values in requested if values[1] is not None}
    deltas = [len(placements), len(added_families - original_families)]
    for metadata, delta in zip(before['physical'], deltas, strict=True):
        old = reference.n(before['raw'], metadata['history'])
        assert old + delta < 2 ** 32
        frames.put(raw, metadata['history'], old + delta)

    result = bytes(raw)
    assert len(result) <= MAX_IMAGE_BYTES
    for logical in before['rows']:
        begin, end = reference.wire.frame(before['raw'], logical >> 8, logical & 255)
        assert result[begin:end] == before['raw'][begin:end]
    for item, requested_row in zip(placements, requested, strict=True):
        begin, end = reference.wire.frame(result, item['page'], item['slot'])
        assert result[begin:end] == reference.record(requested_row)
    trees = []
    for physical, metadata in enumerate(before['physical']):
        nodes, leaves, entries = reference.tree(result, metadata['root'], definition)
        assert set(nodes) == final_members[physical]
        assert entries == index_entries(all_rows, physical)
        trees.append({'physical': physical, 'root': metadata['root'],
                      'height': nodes[metadata['root']]['height'], 'leaves': leaves,
                      'entries': len(entries)})
    for physical in unchanged:
        for page in final_members[physical]:
            assert result[page * PAGE:(page + 1) * PAGE] == before['raw'][page * PAGE:(page + 1) * PAGE]
        begin, end = before['maps']['index' + str(physical)]['range']
        assert result[begin:end] == before['raw'][begin:end]
        counter = before['physical'][physical]['history']
        assert result[counter:counter + 4] == before['raw'][counter:counter + 4]
    return result, {
        'placements': placements,
        'new_data': new_data,
        'allocations': allocations,
        'counter_deltas': deltas,
        'rows': len(all_rows),
        'eligible_secondary': sum(values[1] is not None for values in all_rows.values()),
        'unchanged_physical': unchanged,
        'trees': trees,
        'map_changes': map_changes,
        'image_policy': 'Independent SOURCE+REQUEST engineering bytes; no public AFTER input.',
    }
