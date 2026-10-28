"""Replay saved source/request models, public references and fresh native getters."""
import argparse
import ast
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_multiple_indexes.verify_candidates import Pins, command_report, git_archive
from ace16_insert_ignored_indexes import model, reference, omission_warnings

SOURCE = '.cache/ace16-ignored-index-insert-reference-next-20261028'
FIRST = '.cache/ace16-ignored-index-insert-reference-first-20261028'
CONTINUATION = '.cache/ace16-ignored-index-insert-continuation-next-20261028'
REFERENCE_WORK = '.cache/ace16-ignored-index-insert-continuation-first-20261028'
EXPECTED = '.cache/ace16-ignored-index-insert-expected-first-20261028'
NATIVE = '.cache/ace16-ignored-index-insert-candidates-next-20261028'
NATIVE_WORK = '.cache/ace16-ignored-index-insert-candidates-first-20261028'
GETTERS = '.cache/ace16-ignored-index-insert-getters-next-20261028'
GETTER_WORK = '.cache/ace16-ignored-index-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_ignored_indexes'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/controller-freeze.json': 'fc1aa8b72ec5264e4c5463a444dc46f97726a828a09bdc4ab2306fb567cd1473',
    FIRST + '/report.json': 'c43538a17a74d8be0abbcf96a32d67895e057ccc35fa2ec35d411e48ab30743c',
    CONTINUATION + '/controller-freeze.json': '806f225597c84c31d5af7b0786321dba55ec9ec419f2410e0d7c9ba8d20ccd13',
    REFERENCE_WORK + '/report.json': '2d7442904058e4ac257690048244b9b263fbd5e9d34f8400363e3a1af734c8df',
    EXPECTED + '/freeze.json': 'ff6076fce53ffed22089ad46ab2748cbd409cfb318236c5a1f8d7e3c18b6e0c7',
    NATIVE + '/protocol-freeze.json': '4cadd1cbdcb1480c841acb797efdb2d6ca95cd68175aea15eb65337f258d0a6c',
    NATIVE + '/binding-first.json': '65c34201324fbc6cf3fe677ef07583639c7abd1d3952c007f1a41cb853e0d2bb',
    NATIVE_WORK + '/report.json': '216b3e4bbf21a6760ad06762595cce9618ed296bb6133d750f0a9084a332933f',
    GETTERS + '/controller-freeze.json': '18869f258b4c5c6a296d02255a240af0a0c5cd131fab9d22fe8f8ea269c6da2a',
    GETTERS + '/binding-first.json': '1c6e8790b2fd4684c3eb1974c228b2da180c6c568dbf8cec0141732583f54be7',
    GETTER_WORK + '/report.json': '98364665430015eefa2f74ab524d430a15e5ab14323c9e5ae0affea0250e89e8',
}


def definitions(text, excluded=()):
    return {node.name: ast.dump(node, include_attributes=False)
            for node in ast.parse(text).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name not in excluded}


def maintained_identity(store, pins):
    private = pins.check(store.pin(SOURCE + '/proof.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/reference.py')).read_text()
    assert public == private.replace('ROOT = Path(__file__).resolve().parents[2]',
                                     'ROOT = Path(__file__).resolve().parents[3]')
    assert definitions(private) == definitions(public)
    private = pins.check(store.pin(NATIVE + '/model.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/model.py')).read_text()
    assert public[public.index('def leaf('):] == private[private.index('def leaf('):]
    assert definitions(private, ('load_owner',)) == definitions(public)
    private = pins.check(store.pin(CONTINUATION + '/omission_warning.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/omission_warnings.py')).read_text()
    expected = definitions(private.replace('proof.', 'reference.'))
    assert definitions(public) == {name: expected[name] for name in ('check', 'check_command')}
    public_java = pins.check(store.pin(PUBLIC + '/IgnoredIndexInsertReference.java'))
    executed_java = pins.check(store.pin(SOURCE + '/IgnoredIndexInsertReference.java'))
    assert public_java.read_bytes() == executed_java.read_bytes()


def public_references(store, pins, states):
    plan = store.load(SOURCE + '/plan.json')
    continuation = store.load(CONTINUATION + '/plan.json')
    first = store.load(FIRST + '/report.json')
    later = command_report(store, pins, REFERENCE_WORK + '/report.json', 53, 16 * 1024 * 1024)
    assert first['completed'] is False and first['failure'] == 'AssertionError: '
    assert first['active_command'] == 'V2016-mixed-generate'
    assert first['sources'] == first['references'] == {}
    assert not first['all_sources_admitted_before_append']
    assert len(first['commands']) == 2
    assert first['after_pin_errors'] == first['artifact_pin_errors'] == []
    assert 'assert not (ROOT / capture.records[-1]' in first['traceback']
    for pin in first['before'] + first['artifacts']:
        pins.check(pin)
    assert continuation['commands'] == plan['commands'][2:]
    actual = first['commands'] + later['commands']
    assert len(actual) == len(plan['commands']) == 55
    assert len({call['label'] for call in actual}) == 55
    assert [call['phase'] for call in plan['commands'][:25]].count('before-read') == 16
    assert all(call['phase'] not in ('reference-append', 'after-read') for call in plan['commands'][:25])
    assert later['all_sources_admitted_before_append'] is True
    assert len(later['sources']) == 8 and len(later['references']) == 10
    origins = {case['id']: case for case in plan['origins']}
    cases = {case['id']: case for case in plan['cases']}
    for call, declared in zip(actual, plan['commands'], strict=True):
        assert call['argv'] == declared['argv'] and call['label'] == declared['label']
        assert call['status'] == 0 and call['stopped_for'] is None and call['capture_errors'] == []
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == 16 * 1024 * 1024
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= 16 * 1024 * 1024
        pins.check(call['stdout'])
        stderr = pins.check(call['stderr']).read_bytes()
        if declared['phase'] == 'compile':
            assert not stderr and not store.path(call['stdout']['path']).read_bytes()
        else:
            case = origins[declared['origin']] if 'origin' in declared else cases[declared['case']]
            omission_warnings.check_command(declared, case, stderr)
    results = []
    for case in plan['cases']:
        identity = case['id']
        recorded = later['references'][identity]
        after = reference.inspect(case, pins.check(recorded['image']),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-after-getter.stdout')),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-after-maps.stdout')), True)
        transition = reference.transition(case, states[case['source_id']], after)
        assert recorded['proved'] and recorded['facts'] == reference.describe(after)
        assert transition == recorded['transition']
        results.append({'id': identity, 'transition': transition})
    return {'original_controller_completed': False, 'original_children': 2,
            'continuation_children': 53, 'distinct_children': 55,
            'repeated_completed_calls': 0, 'cases': results}


def replay(root, verify_git=False):
    store = ArtifactStore(root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in [SOURCE + '/controller-freeze.json', CONTINUATION + '/controller-freeze.json',
                 NATIVE + '/protocol-freeze.json', GETTERS + '/controller-freeze.json']:
        pins.closure(path)
    frozen = pins.closure(EXPECTED + '/freeze.json')
    assert frozen['all_eight_source_gates_before_construction'] is True
    assert frozen['committed_new_producer_build_or_execution_inputs'] == 0
    maintained_identity(store, pins)
    source_plan = store.load(SOURCE + '/plan.json')
    plan = store.load(EXPECTED + '/plan.json')
    native_plan = store.load(NATIVE + '/protocol.json')
    getter_plan = store.load(GETTERS + '/protocol.json')
    assert plan['cases'] == native_plan['cases'] == getter_plan['cases']
    assert len(plan['cases']) == 10 and len(source_plan['origins']) == 8
    assert (native_plan['limits']['processes'], getter_plan['limits']['processes']) == (10, 20)
    assert len(getter_plan['commands']) == 20
    assert (native_plan['limits']['native_bytes'],
            native_plan['limits']['native_rows'],
            native_plan['limits']['native_steps']) == (256 * 1024 * 1024, 100000, 100000000)
    assert (native_plan['limits']['request_bytes'],
            native_plan['limits']['request_rows'],
            native_plan['limits']['value_bytes']) == (16 * 1024 * 1024, 4096, 1024)
    recorded_sources = store.load(REFERENCE_WORK + '/report.json')['sources']
    states = {}
    for case in source_plan['origins']:
        identity = case['id']
        state = reference.inspect(case, pins.check(store.pin(case['source'])),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-before-getter.stdout')),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-before-maps.stdout')))
        assert recorded_sources[identity]['admitted'] is True
        assert reference.describe(state) == recorded_sources[identity]['facts']
        states[identity] = state
    modeled = []
    for case in plan['cases']:
        wanted_tsv = ''.join(
            'int32\t' + str(identity) + '\t' +
            ('null\t' if group is None else 'int32\t' + str(group)) +
            '\tbinary\t' + binary.hex() + '\n'
            for identity, group, binary in reference.requests(case['variant'])
        )
        assert pins.check(case['request']).read_text() == wanted_tsv
        expected, details = model.derive(case, states[case['source_id']])
        assert pins.check(case['expected']).read_bytes() == expected
        assert store.load(pins.check(case['facts'])) == details
        modeled.append((expected, details))
    # All source/request image construction precedes any public AFTER replay.
    references = public_references(store, pins, states)
    native = command_report(store, pins, NATIVE_WORK + '/report.json', 10, 8 * 1024 * 1024)
    getters = command_report(store, pins, GETTER_WORK + '/report.json', 20, 16 * 1024 * 1024)
    binding = store.load(NATIVE + '/binding-first.json')
    read_binding = store.load(GETTERS + '/binding-first.json')
    assert binding['revision'] == '1670f7be9943d67fc562aecf0f4a8cd56cdeaec0'
    assert binding['receipt_revision'] == '348b7317bc4eb4d4e64102ce6e35b468a0f8f129'
    assert binding['protocol_freeze'] == store.pin(NATIVE + '/protocol-freeze.json')
    assert read_binding['controller_freeze'] == store.pin(GETTERS + '/controller-freeze.json')
    assert read_binding['producer_report'] == store.pin(NATIVE_WORK + '/report.json')
    assert read_binding['producer_binding'] == store.pin(NATIVE + '/binding-first.json')
    assert binding['workdir'] == NATIVE_WORK and read_binding['workdir'] == GETTER_WORK
    for name in ['artifact', 'build_receipt', 'source_archive']:
        pins.check(binding[name])
        assert binding[name] in native['before']
    receipt = store.load(binding['build_receipt']['path'])
    assert receipt['status'] == 0 and receipt['commit'] == binding['revision']
    assert receipt['artifact'] == binding['artifact'] and receipt['source_archive'] == binding['source_archive']
    assert receipt['expected_images_frozen_before_build'] == store.pin(EXPECTED + '/freeze.json')
    assert len(native['cases']) == len(getters['cases']) == len(read_binding['candidates']) == 10
    historical_artifact = Path(binding['commands'][0][0])
    parts = Path(binding['artifact']['path']).parts
    assert historical_artifact.parts[-len(parts):] == parts
    historical_root = historical_artifact.parents[len(parts) - 1]
    for index, (case, outcome, candidate, (expected, _)) in enumerate(zip(
        plan['cases'], native['cases'], read_binding['candidates'], modeled, strict=True,
    )):
        assert case['id'] == outcome['id'] == candidate['id'] and outcome['matched']
        assert outcome['candidate'] == candidate['image']
        assert candidate['image']['path'] == NATIVE_WORK + '/' + case['id'] + '.accdb'
        assert pins.check(candidate['image']).read_bytes() == expected
        argv = [str(historical_root / binding['artifact']['path']), str(historical_root / case['source']['path']),
                str(historical_root / candidate['image']['path']), 'Items', str(historical_root / case['request']['path'])]
        call = native['commands'][index]
        assert call['argv'] == binding['commands'][index] == argv and call['label'] == case['id']
        assert pins.check(call['stdout']).read_bytes() == f"candidate_changed_rows={case['changed_rows']}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
    results = []
    physical_pairs = 0
    for index, (case, outcome, (_, details), candidate) in enumerate(zip(
        plan['cases'], getters['cases'], modeled, read_binding['candidates'], strict=True,
    )):
        assert outcome['id'] == case['id'] and outcome['matched'] and outcome['whole_image_before_getters']
        assert outcome['candidate'] == candidate['image']
        streams = []
        for call, template, declared in zip(
            getters['commands'][2 * index:2 * index + 2],
            getter_plan['commands'][2 * index:2 * index + 2],
            read_binding['commands'][2 * index:2 * index + 2], strict=True,
        ):
            argv = [str(historical_root / candidate['image']['path']) if token == 'CANDIDATE' else token for token in template['argv']]
            assert call['argv'] == declared['argv'] == argv
            assert call['label'] == template['label'] == declared['label']
            assert pins.check(call['stderr']).read_bytes() == b''
            streams.append(pins.check(call['stdout']))
        after = reference.inspect(case, store.path(candidate['image']['path']), streams[0], streams[1], True)
        transition = reference.transition(case, states[case['source_id']], after)
        assert transition == outcome['transition'] and reference.describe(after) == outcome['facts']
        for placement in details['placements']:
            actual = after['table']['rows'][placement['id']]
            assert (actual['page'], actual['slot']) == (placement['page'], placement['slot'])
            assert after['table']['raw_rows'][placement['id']] == placement['record_hex']
        physical_pairs += sum(tree['entries'] for tree in details['trees'])
        results.append({'id': case['id'], 'rows': after['table']['count'], 'transition': transition,
                        'candidate': candidate['image'], 'unchanged_physical': details['unchanged_physical']})
    git = git_archive(store, pins, binding) if verify_git else None
    pins.terminal()
    return {'available': True, 'verified': True, 'candidate_images': 10,
            'fresh_getter_calls_replayed': 20, 'public_reference_lineage': references,
            'cases': results, 'final_rows': sum(item['rows'] for item in results),
            'old_rows_preserved': sum(item['transition']['source_rows_preserved'] for item in results),
            'inserted_rows': sum(item['transition']['inserted_rows'] for item in results),
            'final_physical_key_ROW_pairs': physical_pairs, 'pin_occurrences': pins.occurrences,
            'distinct_paths': len(pins.items), 'fixture_processes': 0,
            'child_processes': int(verify_git), 'git_verification': git,
            'qualification': 'Engineering candidate and independent-reader evidence, not native Access save/reopen qualification.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true', help='Explicit one Git metadata batch; no fixture processes')
    args = parser.parse_args()
    try:
        result = replay(args.evidence_root, args.verify_git)
    except FileNotFoundError as error:
        print(json.dumps({'available': False, 'verified': False, 'fixture_processes': 0,
                          'child_processes': 0, 'missing': str(error)}, indent=2))
        raise SystemExit(2)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
