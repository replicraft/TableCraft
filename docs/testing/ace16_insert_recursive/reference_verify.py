"""Saved public-reference proofs and first-failure lineage; no subprocesses."""
from . import reference, source

ORIGINAL = '.cache/ace16-insert-parent-growth-actions-next-20261028'
CONTINUED = '.cache/ace16-insert-parent-growth-actions-continuation-next-20261028'
FIRST_WORK = '.cache/ace16-insert-parent-growth-actions-first-20261028'
CONTINUED_WORK = '.cache/ace16-insert-parent-growth-actions-continuation-first-20261028'
MAX_STREAM_BYTES = 16 * 1024 * 1024
MAX_IMAGE_BYTES = 64 * 1024 * 1024
FIRST_FAILURE = "AssertionError: ('Unrelated original prefix byte changed', 3346925, 154, 156)"
FIXED = {
    ORIGINAL + '/freeze.json': '6ca76a68b9f10f42503ce06e55ad0c8c5a7e0644042c86d724e038690ad26d16',
    CONTINUED + '/freeze.json': '5226ed2f22b2d9ff2fca1553ba3d7233e49eee58921eede107a1119d2c2bdc80',
    FIRST_WORK + '/report.json': '2cd5ac22af7ea7b3db2f4d1393190a0d86c92d8de4ce646f041794760f86b916',
    CONTINUED_WORK + '/report.json': '5d93f7d922f02481cd1a597f43564fc7ddd37163cdf1cb8e7e161c7c2dfa26ac',
    CONTINUED + '/first-reference-recheck.json': '5fb455bd666a59bae48c6dec7667abdf37f77684e2771b3552255305901dd009',
}


def maintained(store, check):
    executed = check(store.pin(CONTINUED + '/proof_peer.py')).read_text()
    public = check(store.pin('docs/testing/ace16_insert_recursive/reference.py')).read_text()
    old = "ROOT = Path(__file__).resolve().parents[2]\nSNAPSHOT = ROOT / '.cache/ace16-insert-parent-growth-continuation-next-20261028'\nsys.path.insert(0, str(SNAPSHOT))\nimport snapshot_deleted as snapshot"
    new = "ROOT = Path(__file__).resolve().parents[3]\nsys.path.insert(0, str(ROOT / 'docs/testing'))\nfrom ace16_insert_root_split import reference as root_reference\nfrom ace16_insert_candidates import model as record_codec"
    assert executed.count(old) == 1
    expected = executed.replace(old, new)
    forwarding = '\ndef inspect(case, path, getter, maps):\n    return snapshot.inspect(case, path, getter, maps)\n\n'
    assert expected.count(forwarding) == 1
    expected = expected.replace(forwarding, '\n')
    expected = expected.replace('snapshot.base.CODEC.int_key', 'record_codec.int_key')
    expected = expected.replace('snapshot.base.record', 'root_reference.record')
    expected = expected.replace('snapshot.base.number', 'root_reference.number')
    assert public == expected


def report(store, check, filename, count, completed):
    value = store.load(filename)
    assert value['completed'] is completed and len(value['commands']) == count
    assert value['pin_errors'] == []
    assert value.get('failure') == (None if completed else FIRST_FAILURE)
    for item in value['before'] + value['artifacts']:
        check(item)
    for call in value['commands']:
        assert call['status'] == 0 and call['stopped_for'] is None
        assert call['capture_errors'] == [] and call['timeout_seconds'] == 30
        assert call['stream_limit_bytes'] == MAX_STREAM_BYTES
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= MAX_STREAM_BYTES
        check(call['stdout'])
        assert check(call['stderr']).read_bytes() == b''
    return value


def verify(store, check):
    """Verify public evidence separately from candidate image construction."""
    for filename, digest in FIXED.items():
        item = store.pin(filename)
        assert item['sha256'] == digest, filename
        check(item)
    initial_freeze = store.load(ORIGINAL + '/freeze.json')
    continued_freeze = store.load(CONTINUED + '/freeze.json')
    assert initial_freeze['phase'] == 'public-reference-only-12'
    assert continued_freeze['phase'] == 'remaining-public-reference-only-9'
    for freeze in (initial_freeze, continued_freeze):
        for item in freeze['inputs']:
            check(item)
    maintained(store, check)
    first = report(store, check, FIRST_WORK + '/report.json', 3, False)
    remaining = report(store, check, CONTINUED_WORK + '/report.json', 9, True)
    assert remaining['completed_commands_repeated'] == 0
    assert remaining['new_source_commands'] == remaining['native_candidate_commands'] == 0
    assert first['sources_qualified_before_any_insert'] is True
    assert len(remaining['results']) == 4
    initial_plan = store.load(ORIGINAL + '/plan.json')
    continuation_plan = store.load(CONTINUED + '/plan.json')
    check(store.pin(ORIGINAL + '/plan.json'))
    check(store.pin(CONTINUED + '/plan.json'))
    assert continuation_plan['completed_commands'] == initial_plan['commands'][:3]
    assert continuation_plan['commands'] == initial_plan['commands'][3:]
    actual = first['commands'] + remaining['commands']
    assert len(actual) == 12
    for call, expected in zip(actual, initial_plan['commands'], strict=True):
        assert call['argv'] == expected['argv'] and call['label'] == expected['label']
        assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
        assert expected['timeout_seconds'] == 30 and expected['phase'] == 'reference'
    first_recheck = store.load(CONTINUED + '/first-reference-recheck.json')
    assert first_recheck['saved_only'] and first_recheck['inputs_unchanged']
    assert first_recheck['fixture_commands'] == 0
    for item in first_recheck['inputs']:
        check(item)
    assert [case['id'] for case in initial_plan['cases']] == [
        'V2016-root', 'V2016-internal', 'V2019-root', 'V2019-internal',
    ]
    results = []
    for ordinal, case in enumerate(initial_plan['cases']):
        logs = FIRST_WORK if ordinal == 0 else CONTINUED_WORK
        source_path = check(store.pin(case['source']))
        before_getter = check(store.pin(case['before_getter']))
        before_maps = check(store.pin(case['before_maps']))
        output_path = check(store.pin(case['reference']))
        after_getter = check(store.pin(logs + '/' + case['id'] + '-after-getter.stdout'))
        after_maps = check(store.pin(logs + '/' + case['id'] + '-after-maps.stdout'))
        assert output_path.stat().st_size <= MAX_IMAGE_BYTES
        before = source.inspect(case, source_path, before_getter, before_maps)
        after = source.inspect(case, output_path, after_getter, after_maps)
        observed = reference.compare(case, before, after)
        recorded = remaining['results'][case['id']]
        if ordinal == 0:
            assert recorded['saved_only'] is True
            assert observed == first_recheck['facts'] == recorded['facts']
        else:
            check(recorded['image'])
            recorded_proof = check(recorded['proof'])
            assert store.load(recorded_proof) == observed
        call = actual[ordinal * 3]
        count = before[1]['count']
        assert check(call['stdout']).read_bytes() == f"APPEND\t{case['mode']}\t{count}\t{count + 1}\n".encode()
        assert observed['after_rows'] == observed['before_rows'] + 1
        assert observed['after_history'] == observed['before_history'] + 1
        assert observed['leaf_counts'] == ([228, 227] if case['mode'] == 'root' else [236, 235])
        results.append({'id': case['id'], 'before_rows': count, 'after_rows': count + 1,
                        'history_before': observed['before_history'], 'history_after': observed['after_history'],
                        'new_row': observed['new_row'], 'suffix': observed['suffix'],
                        'public_leaf_counts': observed['leaf_counts'],
                        'public_peer_scratch': observed['public_linked_peer_scratch']})
    return {
        'verified': True, 'public_fixture_commands': 12,
        'initial_successful_commands': 3, 'initial_python_proof_failure_preserved': True,
        'saved_only_corrected_first_reference': True, 'remaining_unique_commands': 9,
        'completed_commands_repeated': 0, 'cases': results,
        'candidate_image_authority': False, 'fixture_processes_executed_by_verifier': 0,
    }
