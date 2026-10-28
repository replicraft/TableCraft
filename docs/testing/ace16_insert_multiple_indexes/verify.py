"""Replay saved independent-writer observations; never start a child process."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_multiple_indexes import reference

ORIGINAL = '.cache/ace16-insert-multiple-index-independent-next-20261028'
CONTINUATION = '.cache/ace16-insert-multiple-index-continuation-next-20261028'
FIRST = '.cache/ace16-insert-multiple-index-reference-first-20261028'
WORK = '.cache/ace16-insert-multiple-index-continuation-first-20261028'
ANCHORS = {
    ORIGINAL + '/controller-freeze.json': '532c34eee7669d543b6c9d707ce96ec0d83899b505751d6866fc1ed54f619383',
    ORIGINAL + '/plan.json': '925f5d31689b73a70bda1cf21e3779acec936dc330c1725a9fb4df2fe831f274',
    FIRST + '/report.json': 'c522a3d4dbc28bae00a0c2abf00337c2f3a4e7baf7b8beadba30aa983d056eeb',
    CONTINUATION + '/freeze.json': '0be65c53c66aa3cb21f1727e1a5527c258b0bb1799c57cf570137f155d5d5952',
    CONTINUATION + '/plan.json': '0685bb29cf2e7efadf4e062f2c2600f658ce9885239a760d773577205a7a61f8',
    WORK + '/report.json': '4cfff792faff4b4fe5e876c708ab9d09bedf4db37dd6b3997aca88b7311fa403',
}


def verify(root):
    store = ArtifactStore(root)
    observed = {}
    occurrences = 0

    def check(pin):
        nonlocal occurrences
        assert store.pin(pin['path']) == pin, pin['path']
        occurrences += 1
        previous = observed.setdefault(pin['path'], pin)
        assert previous == pin
        return store.path(pin['path'])

    for path, digest in ANCHORS.items():
        pin = store.pin(path)
        assert pin['sha256'] == digest, path
        check(pin)
    for path in [ORIGINAL + '/controller-freeze.json', CONTINUATION + '/freeze.json']:
        for pin in store.load(path)['inputs']:
            check(pin)
    plan = store.load(ORIGINAL + '/plan.json')
    continuation = store.load(CONTINUATION + '/plan.json')
    first = store.load(FIRST + '/report.json')
    later = store.load(WORK + '/report.json')
    assert not first['completed'] and first['failure'] == 'AssertionError: '
    assert "assert nodes[metadata['root']]['prefix'] == (1 if index == 0 else 0)" in first['traceback']
    assert len(first['commands']) == 7 and len(first['sources']) == 1 and first['references'] == {}
    assert first['all_sources_admitted_before_append'] is False
    assert later['completed'] and len(later['commands']) == 18
    assert later['lineage'] == continuation['first_report'] == FIRST + '/report.json'
    assert later['all_sources_admitted_before_append'] is True
    assert len(later['sources']) == len(later['references']) == 4
    for report in [first, later]:
        assert report['after_pin_errors'] == report['artifact_pin_errors'] == []
        for pin in report['before'] + report['artifacts']:
            check(pin)
    assert continuation['commands'] == plan['commands'][7:]
    actual = first['commands'] + later['commands']
    assert len(actual) == len(plan['commands']) == 25
    assert len({item['label'] for item in actual}) == 25
    assert plan['commands'][0]['phase'] == 'compile'
    assert all(item['phase'] not in ('reference-append', 'reference-read') for item in plan['commands'][:13])
    for index, (call, declared) in enumerate(zip(actual, plan['commands'], strict=True)):
        assert call['label'] == declared['label'] and call['argv'] == declared['argv']
        assert call['status'] == 0 and call['stopped_for'] is None and call['capture_errors'] == []
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == 16 * 1024 * 1024
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= 16 * 1024 * 1024
        check(call['stdout'])
        assert check(call['stderr']).read_bytes() == b''
        assert declared['argv'][0] == ('javac' if index == 0 else 'java')
        if index:
            assert declared['argv'][1:3] == ['-Xmx512m', '-Dstdout.encoding=UTF-8']
    for source, maximum in [(ORIGINAL, 25), (CONTINUATION, 18)]:
        code = check(store.pin(source + '/bounded.py')).read_text()
        assert f'MAX_PROCESSES = {maximum}\n' in code
        assert 'TIMEOUT_SECONDS = 30\n' in code
        assert 'MAX_STREAM_BYTES = 16 * 1024 * 1024\n' in code
    private = check(store.pin(CONTINUATION + '/proof.py')).read_text()
    public = check(store.pin('docs/testing/ace16_insert_multiple_indexes/reference.py')).read_text()
    original_root = 'ROOT = Path(__file__).resolve().parents[2]'
    assert private.count(original_root) == 1
    assert public == private.replace(original_root, 'ROOT = Path(__file__).resolve().parents[3]')
    java = check(store.pin(ORIGINAL + '/MultiIndexInsertReference.java')).read_bytes()
    assert check(store.pin('docs/testing/ace16_insert_multiple_indexes/MultiIndexInsertReference.java')).read_bytes() == java
    # Source qualification is completed for all four cases before any AFTER
    # replay. These are reference proofs, not candidate image construction.
    sources = {}
    assert [case['id'] for case in plan['cases']] == [
        'V2016-ordinary', 'V2016-both-root-pressure', 'V2019-ordinary', 'V2019-both-root-pressure',
    ]
    for case in plan['cases']:
        folder = FIRST if case['profile'] == 'V2016' else WORK
        sources[case['id']] = reference.inspect(
            case, check(store.pin(case['source'])),
            check(store.pin(folder + '/' + case['id'] + '-before-getter.stdout')),
            check(store.pin(folder + '/' + case['id'] + '-before-maps.stdout')),
        )
        assert reference.describe(sources[case['id']]) == later['sources'][case['id']]
    results = []
    for case in plan['cases']:
        image = check(later['references'][case['id']]['image'])
        assert image == store.path(case['reference'])
        after = reference.inspect(
            case, image,
            check(store.pin(WORK + '/' + case['id'] + '-after-getter.stdout')),
            check(store.pin(WORK + '/' + case['id'] + '-after-maps.stdout')), True,
        )
        transition = reference.transition(case, sources[case['id']], after)
        assert transition == later['references'][case['id']]['transition']
        assert reference.describe(after) == later['references'][case['id']]['facts']
        results.append({'id': case['id'], 'before': reference.describe(sources[case['id']]),
                        'after': reference.describe(after), 'transition': transition})
    for pin in observed.values():
        assert store.pin(pin['path']) == pin
    return {'available': True, 'verified': True, 'fixture_processes': 0, 'child_processes': 0,
            'historical_calls': {'original': 7, 'continuation': 18, 'distinct_total': 25,
                                 'original_python_gate_failed': True, 'completed_calls_repeated': 0},
            'cases': results, 'pin_occurrences': occurrences, 'distinct_paths': len(observed),
            'old_rows_preserved': sum(item['transition']['source_rows_preserved'] for item in results),
            'inserted_rows': sum(item['transition']['inserted_rows'] for item in results),
            'qualification': 'Independent Jackcess public-writer reference only; no Rust candidate or native Access qualification.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    args = parser.parse_args()
    try:
        result = verify(args.evidence_root)
    except FileNotFoundError as error:
        print(json.dumps({'available': False, 'verified': False, 'missing': str(error), 'fixture_processes': 0, 'child_processes': 0}, indent=2))
        raise SystemExit(2)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
