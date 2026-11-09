"""Post-run raw proofs only; never opens a database through Java or performs writes."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

PAGE_BYTES = 4096


def sha(data):
    return hashlib.sha256(data).hexdigest()


def chunk_key(chunk):
    return chunk['reference'], chunk['start'], chunk['end'], chunk['body_sha256']


def live_chunks(state):
    return {chunk_key(chunk): chunk for column in state['fields'].values() for chunk in column['chunks']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--original-facts', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    assert not arguments.output.exists(), 'Use a fresh proof output'
    report_path = arguments.report
    report_before = sha(report_path.read_bytes())
    report = json.loads(report_path.read_text())
    assert report['completed'] and len(report['cases']) == 60
    original_facts = json.loads(arguments.original_facts.read_text())
    original_by_profile = {item['profile']: item for item in original_facts}
    ancestors = {}
    results = []
    transitions, growth = Counter(), Counter()
    for case in report['cases']:
        assert case['accepted_observation']
        selection = case['selection']
        profile = selection['id'].split('-', 1)[0]
        old, new = case['before'], case['after']
        before = Path(selection['source']).read_bytes()
        after = Path(selection['output']).read_bytes()
        assert sha(before) == old['file']['sha256'] and sha(after) == new['file']['sha256']
        inherited = dict(ancestors.get(selection['source'], {}))
        inherited.update(live_chunks(old))
        changed = []
        for key, chunk in inherited.items():
            actual = after[chunk['start']:chunk['end']]
            if sha(actual) != chunk['body_sha256']:
                changed.append({'reference': chunk['reference'], 'before_sha256': chunk['body_sha256'], 'after_sha256': sha(actual)})
        ancestors[selection['output']] = dict(inherited)
        ancestors[selection['output']].update(live_chunks(new))
        map_cells = 0
        for name, prior in old['maps'].items():
            current = new['maps'][name]
            assert prior['payload_type'] == current['payload_type']
            assert (prior['map_page'], prior['map_slot'], prior['row_start'], prior['row_end']) == (current['map_page'], current['map_slot'], current['row_start'], current['row_end'])
            start, end = prior['row_start'], prior['row_end']
            removed = set(prior['all_members']) - set(current['all_members'])
            added = set(current['all_members']) - set(prior['all_members'])
            ranges = []
            if prior['payload_type'] == 0:
                row = before[start:end]
                ranges.append((int.from_bytes(row[1:5], 'little'), start + 5, end))
                assert before[start:start + 5] == after[start:start + 5]
            else:
                assert prior['payload_type'] == 1
                assert before[start:end] == after[start:end]
                row = before[start:end]
                for slot in range((len(row) - 1) // 4):
                    pointer = int.from_bytes(row[1 + 4 * slot:5 + 4 * slot], 'little')
                    if pointer:
                        bitmap_start = pointer * PAGE_BYTES
                        assert before[bitmap_start:bitmap_start + 4] == after[bitmap_start:bitmap_start + 4]
                        ranges.append((slot * (PAGE_BYTES - 4) * 8, bitmap_start + 4, bitmap_start + PAGE_BYTES))
            matched = set()
            for base, first, last in ranges:
                expected = bytearray(before[first:last])
                for page in removed | added:
                    relative = page - base
                    if relative < 0 or relative >= len(expected) * 8:
                        continue
                    index, mask = relative // 8, 1 << (relative % 8)
                    if page in removed:
                        assert expected[index] & mask
                        expected[index] &= ~mask
                    else:
                        assert expected[index] & mask == 0
                        expected[index] |= mask
                    map_cells += 1
                    matched.add(page)
                assert expected == after[first:last], (selection['id'], name)
            assert matched == removed | added
        owned_additions = set()
        for role in ['Table', 'Memo', 'OLE']:
            old_owned = set(old['maps'][role + 'Owned']['all_members'])
            new_owned = set(new['maps'][role + 'Owned']['all_members'])
            assert old_owned <= new_owned
            added = new_owned - old_owned
            owned_additions.update(added)
            old_hint = set(old['maps'][role + 'Hint']['all_members'])
            new_hint = set(new['maps'][role + 'Hint']['all_members'])
            assert new_hint <= new_owned and new_hint - old_hint <= added
        assert owned_additions == set(range(len(before) // PAGE_BYTES, len(after) // PAGE_BYTES))
        old_free = set(old['maps']['GlobalFree']['all_members'])
        new_free = set(new['maps']['GlobalFree']['all_members'])
        assert not new_free - old_free and old_free - new_free == owned_additions
        protected = set()
        for table in original_by_profile[profile]['tables'].values():
            protected.add(table['definition'])
            for tree in table['physical_trees'].values():
                protected.update(tree['pages'])
        for page in protected:
            start, end = page * PAGE_BYTES, (page + 1) * PAGE_BYTES
            assert before[start:end] == after[start:end], (selection['id'], page)
        transitions[case['comparison']['before_kind'] + '->' + case['comparison']['after_kind']] += 1
        growth[str(len(after) - len(before))] += 1
        results.append({'id': selection['id'], 'ancestor_chunk_comparisons': len(inherited),
                        'ancestor_body_changes': changed, 'exact_role_bit_edits': map_cells,
                        'protected_original_td_index_pages': len(protected),
                        'global_allocations_equal_exact_eof_owned_additions': True})
    assert sha(report_path.read_bytes()) == report_before
    result = {'scope': 'Read-only raw-byte re-audit of saved60 public outputs; no new database/action calls.',
              'actions_report_sha256': report_before, 'cases': results,
              'transitions': dict(transitions), 'file_growth_histogram': dict(growth),
              'cumulative_ancestor_chunk_comparisons': sum(item['ancestor_chunk_comparisons'] for item in results),
              'cumulative_ancestor_body_changes': sum(len(item['ancestor_body_changes']) for item in results),
              'exact_map_bit_edits': sum(item['exact_role_bit_edits'] for item in results),
              'completed': True}
    arguments.output.write_text(json.dumps(result, indent=2) + '\n')
    print({key: result[key] for key in ['cumulative_ancestor_chunk_comparisons', 'cumulative_ancestor_body_changes', 'exact_map_bit_edits', 'completed']})


if __name__ == '__main__':
    main()
