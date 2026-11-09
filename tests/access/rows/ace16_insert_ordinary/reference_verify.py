"""Saved ordinary-row public reference proof; never executes a child process."""
from . import reference, source

DIRECTORY = '.cache/ace16-insert-ordinary-reference-next-20261028'
WORK = '.cache/ace16-insert-ordinary-reference-first-20261028'
FREEZE_SHA256 = '22bf0bfdec0aa2eea42ce67975a77a77ac299414f8b4fd271f7fcded5c361b0d'
REPORT_SHA256 = '148b349f871f045e8018909dbe3a094bcb59974a6bcfa9985d466b8dc1c0e71f'
PLAN_SHA256 = '5c1e21634cd1c9ced125af95736de05650bdc3fecbf1f61fa87c2fb80589f58a'
MAX_STREAM_BYTES = 16 * 1024 * 1024
MAX_IMAGE_BYTES = 8 * 1024 * 1024


def fixed(store, check, filename, digest):
    item = store.pin(filename)
    assert item['sha256'] == digest, filename
    return check(item)


def maintained(store, check):
    private = check(store.pin(DIRECTORY + '/proof.py')).read_text()
    public = check(store.pin('docs/testing/ace16_insert_ordinary/reference.py')).read_text()
    old = "ROOT = Path(__file__).resolve().parents[2]\nSOURCE = ROOT / '.cache/ace16-insert-ordinary-qualification-next-20261028'\nMODEL = ROOT / '.cache/ace16-insert-ordinary-actual-model-next-20261028'\nsys.path.insert(0, str(SOURCE))\nsys.path.insert(0, str(MODEL))\nimport source\nimport requests as operands\nimport row_codec\nimport record_decode"
    new = "ROOT = Path(__file__).resolve().parents[3]\nsys.path.insert(0, str(ROOT / 'docs/testing'))\nfrom ace16_insert_ordinary import source, requests as operands, row_codec, record_decode\nfrom ace16_insert_candidates import model as storage_wire\nfrom parent_index_split_originals import wire"
    assert private.count(old) == 1
    expected = private.replace(old, new)
    expected = expected.replace('source.storage_wire.', 'storage_wire.')
    expected = expected.replace('source.wire.', 'wire.')
    assert public == expected


def verify(store, check):
    """Replay all saved public outcomes independently of candidate construction."""
    fixed(store, check, DIRECTORY + '/freeze.json', FREEZE_SHA256)
    fixed(store, check, DIRECTORY + '/plan.json', PLAN_SHA256)
    fixed(store, check, WORK + '/report.json', REPORT_SHA256)
    freeze = store.load(DIRECTORY + '/freeze.json')
    assert freeze['phase'] == 'ordinary-public-reference-only-12'
    assert len(freeze['inputs']) == 70
    for item in freeze['inputs']:
        check(item)
    maintained(store, check)
    plan = store.load(DIRECTORY + '/plan.json')
    protocol = store.load(plan['source_plan'])
    check(store.pin(plan['source_plan']))
    assert plan['commands'] == protocol['future_reference_commands']
    assert len(plan['commands']) == 12
    source_report = store.load(plan['source_report'])
    check(store.pin(plan['source_report']))
    assert source_report['completed'] and source_report['source_qualification'] is False
    assert len(source_report['commands']) == 9 and source_report['pin_errors'] == []
    for actual, expected in zip(source_report['commands'], protocol['source_commands'], strict=True):
        assert actual['argv'] == expected['argv'] and actual['label'] == expected['label']
        assert actual['status'] == 0 and actual['stopped_for'] is None and not actual['capture_errors']
    gates = store.load(plan['source_qualification'])
    check(store.pin(plan['source_qualification']))
    assert gates['qualified'] and len(gates['cases']) == 4
    assert gates['native_operation_admission'] is False
    report = store.load(WORK + '/report.json')
    assert report['completed'] and report['pin_errors'] == []
    assert report['qualified_sources_before_any_insert'] == 4
    assert report['new_source_children'] == report['native_candidate_calls'] == 0
    assert len(report['commands']) == 12 and len(report['results']) == 4
    for item in report['before'] + report['artifacts']:
        check(item)
    assert store.pin(DIRECTORY + '/freeze.json') in report['before']
    for call, expected in zip(report['commands'], plan['commands'], strict=True):
        assert call['label'] == expected['label'] and call['argv'] == expected['argv']
        assert expected['phase'] == 'reference' and expected['timeout_seconds'] == 30
        assert call['argv'][:5] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8',
                                  '-Duser.timezone=UTC',
                                  '-Dcom.healthmarketscience.jackcess.dateTimeType=LOCAL_DATE_TIME']
        assert call['status'] == 0 and call['stopped_for'] is None
        assert call['capture_errors'] == [] and call['timeout_seconds'] == 30
        assert call['stream_limit_bytes'] == MAX_STREAM_BYTES
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= MAX_STREAM_BYTES
        check(call['stdout'])
        assert check(call['stderr']).read_bytes() == b''
    cases = plan['cases']
    assert [case['id'] for case in cases] == [
        'V2016-FixedRows', 'V2016-MixedRows', 'V2019-FixedRows', 'V2019-MixedRows',
    ]
    # All BEFORE facts are qualified before any public AFTER projection.
    original = {}
    for case in cases:
        original[case['id']] = source.inspect(case, check(store.pin(case['source'])),
                                               check(store.pin(case['before_getter'])),
                                               check(store.pin(case['before_maps'])))
    results = []
    for ordinal, case in enumerate(cases):
        recorded = report['results'][case['id']]
        image = check(recorded['image'])
        assert recorded['image'] == store.pin(case['reference'])
        assert 0 < image.stat().st_size <= MAX_IMAGE_BYTES
        after = reference.inspect(case, image,
                                  check(store.pin(WORK + '/' + case['id'] + '-getter.stdout')),
                                  check(store.pin(WORK + '/' + case['id'] + '-maps.stdout')),
                                  original[case['id']])
        measured = reference.transition(case, original[case['id']], after)
        assert store.load(check(recorded['proof'])) == measured
        insertion = report['commands'][ordinal * 3]
        assert check(insertion['stdout']).read_bytes() == f"APPENDED\t{case['table']}\t451\t452\n".encode()
        assert measured['rows_before'] == measured['history_before'] == 451
        assert measured['rows_after'] == measured['history_after'] == 452
        assert measured['new_ROW']['bytes'] == (1106 if case['table'] == 'MixedRows' else 71)
        assert sum(item['purpose'] == 'DATA' for item in measured['suffix']) == int(case['table'] == 'MixedRows')
        assert sum(item['purpose'] == 'INDEX' for item in measured['suffix']) == 2
        assert measured['candidate_image_authority'] is False
        results.append({'id': case['id'], 'new_ROW': measured['new_ROW'],
                        'suffix': measured['suffix'], 'root': measured['root'],
                        'history_before': measured['history_before'], 'history_after': measured['history_after']})
    return {'verified': True, 'public_reference_commands': 12, 'public_reference_proofs': 4,
            'source_construction_repeated': 0, 'candidate_image_authority': False,
            'first_attempt': True, 'cases': results, 'fixture_processes_executed_by_verifier': 0}
