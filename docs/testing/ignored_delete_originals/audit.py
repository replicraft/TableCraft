"""Independent raw transition audit of the fifty public-writer reference copies."""

from pathlib import Path
import hashlib
import json
import argparse

from support import REPOSITORY_ROOT, fresh_report, pin, wire

GLOBAL_REFERENCE = 256
MAP_BASE_START = 1
INLINE_BITS_START = 5
INDIRECT_POINTER_START = 1
BITMAP_BODY_START = 4
POINTER_BYTES = 4
BITS_PER_BYTE = 8
BITMAP_BITS = (wire.PAGE_SIZE - BITMAP_BODY_START) * BITS_PER_BYTE
SLOT_START = 14
SLOT_WIDTH = 2
SLOT_OFFSET_MASK = 0x1FFF
TOMBSTONE_FLAGS = 0xC000
RETIRED_PREFIX_BYTES = 4
OWNED_MAP_POINTER_OFFSET = 55


def map_state(raw, reference, owner):
    container = reference >> wire.REFERENCE_SLOT_BITS
    assert wire.u32(raw, container * wire.PAGE_SIZE + wire.PAGE_OWNER_OFFSET) == owner
    payload = wire.row(raw, reference)
    storage = [container]
    if payload[0] == wire.INLINE_MAP:
        base = wire.u32(payload, MAP_BASE_START)
        bits = {base + byte * BITS_PER_BYTE + bit
                for byte, mask in enumerate(payload[INLINE_BITS_START:])
                for bit in range(BITS_PER_BYTE) if mask & (1 << bit)}
        layout = ('inline', payload[:INLINE_BITS_START].hex(), len(payload))
    else:
        assert payload[0] == wire.INDIRECT_MAP and (len(payload) - INDIRECT_POINTER_START) % POINTER_BYTES == 0
        bits = set()
        for block in range((len(payload) - INDIRECT_POINTER_START) // POINTER_BYTES):
            pointer = wire.u32(payload, INDIRECT_POINTER_START + block * POINTER_BYTES)
            if pointer:
                page = wire.page(raw, pointer)
                assert page[:BITMAP_BODY_START] == wire.BITMAP_SIGNATURE
                storage.append(pointer)
                bits.update(block * BITMAP_BITS + byte * BITS_PER_BYTE + bit
                            for byte, mask in enumerate(page[BITMAP_BODY_START:])
                            for bit in range(BITS_PER_BYTE) if mask & (1 << bit))
        layout = ('indirect', payload.hex())
    return bits, layout, storage


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--references', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists()
    args.report.resolve().relative_to(REPOSITORY_ROOT)
    pin(args.references)
    original_report = args.references
    report = json.loads(original_report.read_text())
    originals = {(item['family'], item['profile']): item for item in report['originals']}
    results = []
    for case in report['results']:
        source, target = Path(case['source']['path']), Path(case['reference']['path'])
        assert pin(source) == case['source'] and pin(target) == case['reference']
        before, after = source.read_bytes(), target.read_bytes()
        assert len(after) >= len(before)
        original = originals[(case['family'], case['profile'])]['tables'][case['table']]
        global_before, global_layout, global_storage = map_state(before, GLOBAL_REFERENCE, 1)
        global_after, after_layout, after_storage = map_state(after, GLOBAL_REFERENCE, 1)
        global_storage_added = set(after_storage) - set(global_storage)
        global_storage_retired = set(global_storage) - set(after_storage)
        distinct = {}
        for transition in case['transitions']:
            distinct.setdefault(transition['physical'], transition)
        retired, added = set(), set()
        map_storage_added, map_storage_retired = set(), set()
        transitions = []
        for physical, transition in distinct.items():
            previous = original['indexes'][transition['index']]
            old_owned = set(transition['owned_before'])
            new_owned = set(transition['owned_after'])
            assert transition['root'] in new_owned
            added.update(new_owned - old_owned)
            removed = old_owned - new_owned
            assert not retired.intersection(removed)
            retired.update(removed)
            old_bits, layout, storage = map_state(before, previous['usage'], 0)
            new_bits, new_layout, new_storage = map_state(after, previous['usage'], 0)
            map_storage_added.update(set(new_storage) - set(storage))
            map_storage_retired.update(set(storage) - set(new_storage))
            assert old_bits - new_bits == removed
            assert new_bits - old_bits == new_owned - old_owned
            assert not old_owned.intersection(global_before)
            assert not new_owned.intersection(global_after)
            assert not set(storage).intersection(global_before)
            retired_observations = []
            for page in removed:
                old_page, new_page = wire.page(before, page), wire.page(after, page)
                assert new_page[:RETIRED_PREFIX_BYTES] == bytes(RETIRED_PREFIX_BYTES)
                old_residual = old_page[RETIRED_PREFIX_BYTES:]
                new_residual = new_page[RETIRED_PREFIX_BYTES:]
                retired_observations.append({
                    'page': page,
                    'original_residual_sha256': hashlib.sha256(old_residual).hexdigest(),
                    'reference_residual_sha256': hashlib.sha256(new_residual).hexdigest(),
                    'changed_residual_bytes': sum(a != b for a, b in zip(old_residual, new_residual)),
                    'reference_prefix_zero': True,
                })
            transitions.append({
                'physical': physical, 'root': transition['root'], 'retired': sorted(removed),
                'height_before': transition['height_before'], 'height_after': transition['height_after'],
                'entries_before': transition['original_entries'], 'entries_after': transition['remaining_entries'],
                'effective_deleted': transition['effective_deleted'],
                'effective_empty': transition['effective_empty'],
                'index_pages_byte_identical': transition['pages_byte_identical'],
                'index_empty_with_live_table_rows': transition['index_empty_with_live_table_rows'],
                'map_representation_before': layout[0],
                'map_representation_after': new_layout[0],
                'new_owned_pages': sorted(new_owned - old_owned),
                'map_storage_before': storage, 'map_storage_after': new_storage,
                'retired_residual_observations': retired_observations,
            })
        expected_free = retired | map_storage_retired | global_storage_retired
        expected_used = added | map_storage_added | global_storage_added
        assert global_after - global_before == expected_free
        assert global_before - global_after == expected_used
        file_pages = max(len(before), len(after)) // wire.PAGE_SIZE
        assert {page for page in global_before if page >= file_pages} == {
            page for page in global_after if page >= file_pages}
        assert not set(global_storage).intersection(global_before)
        data_reference = wire.u32(before, original['definition'] * wire.PAGE_SIZE + OWNED_MAP_POINTER_OFFSET)
        assert wire.row(before, data_reference) == wire.row(after, data_reference)
        for identifier, row in original['rows'].items():
            number, slot = row['page'], row['slot']
            offset = number * wire.PAGE_SIZE + SLOT_START + slot * SLOT_WIDTH
            old_word, new_word = wire.u16(before, offset), wire.u16(after, offset)
            assert new_word == (old_word | TOMBSTONE_FLAGS if int(identifier) in case['selected_ids'] else old_word)
            payload = wire.row(before, (number << wire.REFERENCE_SLOT_BITS) | slot)
            start = number * wire.PAGE_SIZE + (old_word & SLOT_OFFSET_MASK)
            assert after[start:start + len(payload)] == payload
        results.append({
            'family': case['family'], 'profile': case['profile'], 'table': case['table'],
            'selection': case['selection'], 'deleted': case['deleted'], 'remaining': case['remaining'],
            'retired': sorted(retired), 'new_owned_pages': sorted(added),
            'file_bytes_before': len(before), 'file_bytes_after': len(after),
            'new_map_storage': sorted(map_storage_added | global_storage_added),
            'distinct_physical_indexes': transitions,
            'unchanged_data_owned_map': True, 'all_original_record_payloads_preserved': True,
            'exact_global_free_bit_delta': True, 'global_bits_beyond_both_file_extents_preserved': True,
        })
    output = {'scope': 'Independent raw allocation/slot/payload transition audit of public copies; no kernel candidates',
              'source_report': pin(original_report), 'audit_source': pin(Path(__file__)),
              'references': len(results), 'results': results,
              'reference_residual_note': (
                  'Public sequential deleteRow may rewrite a page during earlier deletes before retiring it. '
                  'Changed residual bytes are observed, not treated as a TableCraft policy relaxation; '
                  'candidate preservation must compare original-to-candidate separately.')}
    fresh_report(args.report, output)
    print('audited references', len(results), 'retired pages', sum(len(x['retired']) for x in results), flush=True)
    print('empty indexes with live table rows', sum(t['index_empty_with_live_table_rows']
          for x in results for t in x['distinct_physical_indexes']), flush=True)


if __name__ == '__main__':
    main()
