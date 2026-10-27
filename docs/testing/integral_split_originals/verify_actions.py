"""Recheck saved typed MAX references without generation or fixture processes."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from integral_split_originals import action_model
from integral_split_originals.verify_sources import all_pins, check_pin, normalized

TARGETS = {'ByteKeys': 255, 'Int16Keys': 32767,
           'Int64Keys': (1 << 63) - 1, 'CurrencyKeys': (1 << 63) - 1}
PAGE_BYTES = 4096
FREE_OFFSET = 2
FREE_BYTES = 2


def verify_framing(path):
    saved = json.loads(path.read_text())
    list(all_pins(saved))
    assert saved['read_only_saved_copy_analysis']
    assert saved['fixture_processes_repeated'] == 0
    fresh_pages = 0
    for case in saved['cases']:
        before = check_pin(case['source']).read_bytes()
        after = check_pin(case['reference']).read_bytes()
        assert set(case['physical']) == {'1', '2'}
        for effect in case['physical'].values():
            offset = effect['root'] * PAGE_BYTES
            end = effect['root_active_end']
            assert before[offset + end:offset + PAGE_BYTES] == after[offset + end:offset + PAGE_BYTES]
            assert effect['old_root_suffix_changed_bytes'] == 0
            assert effect['changes_outside_engineering_root_spans'] == []
            for page in effect['fresh_pages']:
                offset = page['page'] * PAGE_BYTES
                body = after[offset:offset + PAGE_BYTES]
                active_end = PAGE_BYTES - int.from_bytes(body[FREE_OFFSET:FREE_OFFSET + FREE_BYTES], 'little')
                assert active_end == page['active_end']
                assert sum(value != 0 for value in body[active_end:]) == page['unused_nonzero_bytes']
                assert page['unused_nonzero_bytes'] > 0
                fresh_pages += 1
    assert len(saved['cases']) == 8 and fresh_pages == 32
    return fresh_pages


def verify(path):
    original_bytes = path.read_bytes()
    report = json.loads(original_bytes)
    pins = list(all_pins(report))
    assert report['generation_calls'] == report['new_split_api_calls'] == 0
    commands = report['commands']
    assert len(commands) == 25 and len(report['cases']) == 8
    by_label = {item['label']: item for item in commands}
    assert len(by_label) == len(commands)
    assert by_label['compile']['status'] == 0
    sources = json.loads((ROOT / '.cache/ace16-integral-split-sources-first-20261027/report.json').read_text())
    originals = {item['profile']: item for item in sources['profiles']}
    identities = set()
    physical_splits = 0
    for case in report['cases']:
        profile, target = case['profile'], case['target']
        assert (profile, target) not in identities
        identities.add((profile, target))
        assert case['expected_typed_max_units'] == TARGETS[target]
        assert case['source'] == case['source_after'] == originals[profile]['original']
        stem = profile + '-' + target
        native = by_label[stem + '-native']
        assert native['status'] == 1 and native['stopped_for'] is None
        assert check_pin(native['stdout']).read_bytes() == b''
        assert 'Unsupported(IndexedColumn)' in check_pin(native['stderr']).read_text()
        assert not (ROOT / native['argv'][2]).exists()
        assert case['native_attempt']['candidate'] is None
        assert case['native_attempt']['indexed_column_refusal_observed']
        public, describe = by_label[stem + '-public'], by_label[stem + '-describe']
        assert all(item['status'] == 0 and item['stopped_for'] is None
                   for item in (public, describe))
        actual = action_model.analyze(check_pin(case['source']), check_pin(case['reference']),
                                      originals[profile], target,
                                      check_pin(describe['stdout']).read_text())
        assert normalized(actual) == case['analysis']
        assert actual['eof_page_delta'] == 4
        assert actual['row_reference_changes'] == actual['counter_changes'] == []
        assert actual['unaffected_index_rewrites'] == []
        assert len(actual['raw_row_body_changes']) == 1
        assert actual['raw_row_body_changes'][0]['exact_selected_field_only']
        effects = actual['selected_index_effects']
        assert effects['Ascending'] == effects['Alias']
        for label, physical in [('Ascending', 1), ('Descending', 2)]:
            effect = effects[label]
            assert effect['physical'] == physical
            assert effect['before_root'] == effect['after_root']
            assert effect['root_height'] == 1 and len(effect['root_entries_hex']) == 1
            assert len(effect['owned_added']) == 2 and effect['owned_removed'] == []
            assert effect['complete_owned_added'] == effect['owned_added']
            assert effect['complete_owned_removed'] == []
            assert effect['counter_before'] == effect['counter_after']
            assert len(effect['leaves']) == 2
            expected_count = 450 if target in ('ByteKeys', 'Int16Keys') else 200
            assert all(leaf['count'] == expected_count and leaf['prefix'] == 0
                       for leaf in effect['leaves'])
            physical_splits += 1
    assert identities == {(profile, target) for profile in ('V2016', 'V2019')
                          for target in TARGETS}
    post = json.loads((path.parent / 'post-execution-pins.json').read_text())
    post_pins = list(all_pins(post))
    assert post['exit_code'] == post['mismatches'] == 0
    assert post['direct_pin_occurrences'] == 25
    assert post['report_before'] == post['report_after']
    assert post['report_after']['sha256'] == hashlib.sha256(original_bytes).hexdigest()
    assert all(item['matches'] and item['expected'] == item['actual'] for item in post['checks'])
    assert path.read_bytes() == original_bytes
    fresh_pages = verify_framing(path.parent / 'framing.json')
    return {'read_only': True, 'fixture_processes_repeated': 0,
            'historical_fixture_processes': 25, 'public_updates': 8,
            'old_consumer_atomic_refusals': 8, 'physical_splits': physical_splits,
            'raw_pin_occurrences': len(pins), 'postcheck_pin_occurrences': len(post_pins),
            'public_fresh_pages_with_nonzero_unused_scratch': fresh_pages,
            'raw_report_unchanged': True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    path = (ROOT / args.report).resolve()
    path.relative_to(ROOT)
    print(json.dumps(verify(path), indent=2))


if __name__ == '__main__':
    main()
