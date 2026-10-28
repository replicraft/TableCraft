"""Replay saved source/request models, public references and fresh native getters."""
import argparse
import ast
import json
from pathlib import Path
import sys
from uuid import UUID

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_multiple_indexes.verify_candidates import Pins, command_report, git_archive
from ace16_insert_variable_indexes import model, reference, recipes, stderr_policy

SOURCE = '.cache/ace16-variable-key-index-insert-reference-continuation-next-20261028'
SOURCE_PLAN = '.cache/ace16-variable-key-index-insert-source-checked-next-20261028'
REFERENCE_CONT_WORK = '.cache/ace16-variable-key-index-insert-reference-continuation-first-20261028'
GUARDS = '.cache/ace16-variable-key-index-insert-guards-next-20261028'
GUARD_WORK = '.cache/ace16-variable-key-index-insert-guards-first-20261028'
REFERENCE_WORK = '.cache/ace16-variable-key-index-insert-reference-checked-first-20261028'
EXPECTED = '.cache/ace16-variable-key-index-insert-expected-first-20261028'
NATIVE = '.cache/ace16-variable-key-index-insert-candidates-next-20261028'
NATIVE_WORK = '.cache/ace16-variable-key-index-insert-candidates-first-20261028'
GETTERS = '.cache/ace16-variable-key-index-insert-getters-next-20261028'
GETTER_WORK = '.cache/ace16-variable-key-index-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_variable_indexes'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/controller-freeze.json': '587fbdbf2e248c6998e8b1c2082f1e14bf157b72573c5bd227ef2f4186671008',
    SOURCE_PLAN + '/controller-freeze.json': '764cce94b434bd1341582dbbc0408b71f6ecbadfc18db354141c311d2471b53f',
    REFERENCE_WORK + '/report.json': 'ec910f53760912fa75fc7b32d50a0de23a435fb14c4fd80d7ffdb5d7b9649740',
    REFERENCE_CONT_WORK + '/report.json': 'e5a72d4f10786bde293391b4ec15db75bb37a0f60158f2c2026b23fa88f6564d',
    EXPECTED + '/freeze.json': 'ccb82262dddf1340b7a0e1e8ae652524b24f94edc1294cfe319ab615fda61429',
    NATIVE + '/protocol-freeze.json': 'cdad6237e9c53f3ceaa87e659e2c82012677e1c54dff03e8bde55f0687dee94a',
    NATIVE + '/binding-first.json': '360a7b060a85272ed2f44b1b8c63b21c1ca9a215532a9f19aa154eac103ff0fe',
    NATIVE_WORK + '/report.json': 'f352f6ab8253c006e09c268dc0ff50efea2664bed8a4a4979340c7adec28b083',
    GUARDS + '/controller-freeze.json': '5e49d6644d1cadc8fa2a942b4ea81bda111a83ea8bc677c285f7e57ef5ee6225',
    GUARDS + '/binding-first.json': '09fbbc15f671008a1d4a2df090fc8ac384774ec016d4b1d760a9c2a1a15b53f1',
    GUARD_WORK + '/report.json': '816c232fda7885e650a1ced7aad52f5f3e58bde7a02959f37de202dcd8df9c69',
    GETTERS + '/controller-freeze.json': 'da9cafbb1c31233483bd043e94a0965b76fb7ac557172ddb3c928319b75da8c6',
    GETTERS + '/binding-first.json': '41379aaa6806d8232e9765ce0d79a2d9aed6c3c049e2ad5fd468031227d1c262',
    GETTER_WORK + '/report.json': 'bb5a462f5f3d0cc08d9473f3db8432a97fa2909dcc7a3b598bcbaa108e390387',
}


def definitions(text, excluded=()):
    return {node.name: ast.dump(node, include_attributes=False)
            for node in ast.parse(text).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name not in excluded}


def maintained_identity(store, pins):
    private = pins.check(store.pin(SOURCE + '/proof.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/reference.py')).read_text()
    assert public == private.replace('ROOT = Path(__file__).resolve().parents[2]',
                                     'ROOT = Path(__file__).resolve().parents[3]').replace('import recipes\n', 'from ace16_insert_variable_indexes import recipes\n')
    assert definitions(private) == definitions(public)
    private = pins.check(store.pin(NATIVE + '/model.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/model.py')).read_text()
    assert public[public.index('def n('):] == private[private.index('def n('):]
    assert definitions(private, ('load_owner',)) == definitions(public)
    public_java = pins.check(store.pin(PUBLIC + '/VariableKeyIndexInsertReference.java'))
    executed_java = pins.check(store.pin(SOURCE_PLAN + '/VariableKeyIndexInsertReference.java'))
    assert public_java.read_bytes() == executed_java.read_bytes()

    assert pins.check(store.pin(PUBLIC + '/recipes.py')).read_bytes() == pins.check(store.pin(SOURCE + '/recipes.py')).read_bytes()
    private = pins.check(store.pin(SOURCE_PLAN + '/stderr_policy.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/stderr_policy.py')).read_text()
    assert public == private.replace('parents[2]', 'parents[3]').replace('import recipes\n', 'from ace16_insert_variable_indexes import recipes\n')
    assert definitions(private) == definitions(public)




def retained_report(store, pins, path, count, completed, expected_status):
    report = store.load(path)
    assert report['completed'] is completed and len(report['commands']) == count
    assert report['after_pin_errors'] == report['artifact_pin_errors'] == []
    for pin in report['before'] + report['artifacts']:
        pins.check(pin)
    for call in report['commands']:
        assert call['status'] == expected_status and call['stopped_for'] is None
        assert call['capture_errors'] == [] and call['timeout_seconds'] == 30
        assert call['stream_limit_bytes'] == 16 * 1024 * 1024
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= 16 * 1024 * 1024
        pins.check(call['stdout'])
        pins.check(call['stderr'])
    return report


def public_references(store, pins, states):
    plan = store.load(SOURCE_PLAN + '/plan.json')
    failed_paths = (
        '.cache/ace16-variable-key-index-insert-reference-first-20261028/report.json',
        '.cache/ace16-variable-key-index-insert-reference-correction-first-20261028/report.json',
    )
    diagnostics = ('void cannot be converted to ColumnBuilder', 'unreported exception IOException')
    for path, diagnostic in zip(failed_paths, diagnostics, strict=True):
        failed = retained_report(store, pins, path, 1, False, 1)
        assert failed['sources'] == failed['references'] == {}
        call = failed['commands'][0]
        assert call['label'] == 'compile' and call['argv'][0] == 'javac'
        assert diagnostic in pins.check(call['stderr']).read_text()
    stopped = retained_report(store, pins, REFERENCE_WORK + '/report.json', 133, False, 0)
    assert 'entries[-1] < complete' in stopped['traceback']
    assert len(stopped['sources']) == 42 and len(stopped['references']) == 1
    assert stopped['all_raw_source_gates_before_reference_append'] is True
    continuation = command_report(store, pins, REFERENCE_CONT_WORK + '/report.json', 120, 16 * 1024 * 1024)
    assert len(continuation['references']) == 40 and continuation['sources_requalified'] == 42
    calls = stopped['commands'] + continuation['commands']
    assert len(calls) == len(plan['commands']) == 253
    assert len({call['label'] for call in calls}) == 253
    assert plan['commands'][127]['phase'] == 'reference-append'
    assert [call['label'] for call in continuation['commands']] == [call['label'] for call in plan['commands'][133:]]
    cases = {case['id']: case for case in plan['cases']}
    warnings = []
    for call, declared in zip(calls, plan['commands'], strict=True):
        assert call['argv'] == declared['argv'] and call['label'] == declared['label']
        case = cases.get(declared.get('case', declared.get('origin')))
        count = stderr_policy.check(declared, case, pins.check(call['stderr']).read_bytes())
        if count:
            warnings.append({'label': call['label'], 'count': count})
    assert warnings == stopped['omission_warning_checks'] + continuation['omission_warning_checks']
    assert sum(item['count'] for item in warnings) == 18
    corrected = store.load(pins.check(store.pin(SOURCE + '/saved-first-two-proof.json')))
    assert corrected['completed'] is True and corrected['raw_report_completed'] is False
    assert corrected['source_only_gates'] == 42 and len(corrected['cases']) == 2
    results = []
    for index, case in enumerate(plan['cases']):
        identity = case['id']
        work = REFERENCE_WORK if index < 2 else REFERENCE_CONT_WORK
        after = reference.inspect(case, pins.check(store.pin(case['reference'])),
            pins.check(store.pin(work + '/' + identity + '-after-getter.stdout')),
            pins.check(store.pin(work + '/' + identity + '-after-maps.stdout')),
            encoder_path=pins.check(store.pin(case['encoder'])), after=True)
        transition = reference.transition(case, states[identity], after)
        facts = reference.describe(after)
        if index < 2:
            assert corrected['cases'][index]['facts'] == facts
            assert corrected['cases'][index]['transition'] == transition
            if index == 0:
                assert stopped['references'][identity]['facts'] == facts
                assert stopped['references'][identity]['transition'] == transition
        else:
            recorded = continuation['references'][identity]
            assert recorded['proved'] is True and recorded['facts'] == facts
            assert recorded['transition'] == transition
        results.append({'id': identity, 'transition': transition})
    return {'planned_distinct_completed_calls': 253, 'failed_compile_calls': 2,
            'actual_children_including_failed_compiles': 255,
            'original_stopped_calls': 133, 'original_report_completed': False,
            'corrective_saved_reference_proofs': 2, 'continuation_calls': 120,
            'all_raw_source_gates_before_append': 42, 'prospective_positive_origins': 38,
            'intentional_native_guard_origins': 4, 'public_transition_proofs': 42,
            'repeated_completed_calls': 0, 'first_overall_pass': False, 'cases': results}


def verify_guards(store, pins, binding, historical_root):
    report = store.load(GUARD_WORK + '/report.json')
    assert report['completed'] is True and not report.get('failure')
    assert report['after_pin_errors'] == report['artifact_pin_errors'] == []
    assert len(report['commands']) == len(report['cases']) == 4
    plan = store.load(GUARDS + '/protocol.json')
    guard_binding = store.load(pins.check(report['binding']))
    assert guard_binding['protocol_freeze'] == store.pin(GUARDS + '/controller-freeze.json')
    assert guard_binding['positive_report'] == store.pin(NATIVE_WORK + '/report.json')
    assert guard_binding['positive_binding'] == store.pin(NATIVE + '/binding-first.json')
    for pin in report['before'] + report['artifacts']:
        pins.check(pin)
    for name in ('artifact', 'build_receipt', 'source_archive'):
        assert binding[name] in report['before']
    for case, outcome, call, declared in zip(plan['cases'], report['cases'], report['commands'], guard_binding['commands'], strict=True):
        assert case['id'] == outcome['id'] == call['label'] and outcome['matched'] is True
        assert outcome['output_absent'] is True and case['expected_image'] is None
        output = GUARD_WORK + '/' + case['id'] + '.accdb'
        assert not store.path(output).exists()
        argv = [str(historical_root / binding['artifact']['path']), str(historical_root / case['source']['path']),
                str(historical_root / output), 'Items', str(historical_root / case['request']['path'])]
        assert call['argv'] == declared == argv
        assert call['status'] == call['expected_status'] == 1
        assert call['stopped_for'] is None and call['capture_errors'] == []
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == 8 * 1024 * 1024
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= 8 * 1024 * 1024
        assert pins.check(call['stdout']).read_bytes() == b''
        assert pins.check(call['stderr']).read_bytes() == b'Error: Unsupported(ConstraintCoverage)\n'
        pins.check(case['source'])
        pins.check(case['request'])
    return {'calls': 4, 'actual_statuses': [1] * 4, 'exact_ConstraintCoverage': True,
            'outputs_published': 0, 'first_attempt_completed': True}


def certify_expected(case, before, expected, facts):
    requested = recipes.requests(case)
    assert model.requests(case) == requested
    all_rows = dict(before['rows'])
    for placement, values in zip(facts['placements'], requested, strict=True):
        page, slot = placement['page'], placement['slot']
        begin, end = reference.wire.frame(expected, page, slot)
        assert reference.decode_record(expected[begin:end], before['layout'], case) == values
        logical = page * 256 + slot
        assert logical not in all_rows
        all_rows[logical] = values
    definition = before['table']['definition']
    for physical, metadata in enumerate(before['physical']):
        _, _, entries = reference.tree(expected, metadata['root'], definition, case, physical == 0)
        wanted = sorted((recipes.encoded_key(values, case, physical == 0) + logical.to_bytes(4, 'big')
                         for logical, values in all_rows.items() if physical == 0 or recipes.eligible(values, case)), key=reference.entry_order)
        assert entries == wanted



def replay(root, verify_git=False):
    store = ArtifactStore(root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in [SOURCE + '/controller-freeze.json', SOURCE_PLAN + '/controller-freeze.json', GUARDS + '/controller-freeze.json',
                 NATIVE + '/protocol-freeze.json', GETTERS + '/controller-freeze.json']:
        pins.closure(path)
    frozen = pins.closure(EXPECTED + '/freeze.json')
    assert frozen['all_forty_two_source_gates_before_positive_construction'] is True
    assert frozen['committed_new_producer_build_or_execution_inputs'] == 0
    maintained_identity(store, pins)
    source_plan = store.load(SOURCE_PLAN + '/plan.json')
    plan = store.load(EXPECTED + '/plan.json')
    native_plan = store.load(NATIVE + '/protocol.json')
    getter_plan = store.load(GETTERS + '/protocol.json')
    assert plan['cases'] == native_plan['cases'] == getter_plan['cases']
    assert len(plan['cases']) == 38 and len(source_plan['origins']) == 42
    assert [case['id'] for case in plan['cases']] == [case['id'] for case in source_plan['cases'] if case['native_outcome'] == 'image']
    assert (native_plan['limits']['processes'], getter_plan['limits']['processes']) == (38, 76)
    assert len(getter_plan['commands']) == 76
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
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-before-maps.stdout')),
            encoder_path=pins.check(store.pin(case['encoder'])))
        assert recorded_sources[identity]['raw_source_qualified'] is True
        assert recorded_sources[identity]['native_write_admission_claimed'] is False
        assert reference.describe(state) == recorded_sources[identity]['facts']
        states[identity] = state
    modeled = []
    for case in plan['cases']:
        lines = []
        for identity, group, payload in recipes.requests(case):
            fields = ['int32', str(identity)]
            fields.extend(['null', ''] if group is None else ['int32', str(group)])
            if payload is None:
                fields.extend(['null', ''])
            elif case['family'] == 'Binary':
                fields.extend(['binary', payload.hex()])
            else:
                fields.extend(['text', payload.decode('ascii')])
            lines.append('\t'.join(fields) + '\n')
        assert pins.check(case['request']).read_text() == ''.join(lines)
        expected, details = model.derive(case, states[case['source_id']])
        certify_expected(case, states[case['source_id']], expected, details)
        assert pins.check(case['expected']).read_bytes() == expected
        assert store.load(pins.check(case['facts'])) == details
        modeled.append((expected, details))
    # All source/request image construction precedes any public AFTER replay.
    references = public_references(store, pins, states)
    native = command_report(store, pins, NATIVE_WORK + '/report.json', 38, 8 * 1024 * 1024)
    getters = command_report(store, pins, GETTER_WORK + '/report.json', 76, 16 * 1024 * 1024)
    binding = store.load(NATIVE + '/binding-first.json')
    read_binding = store.load(GETTERS + '/binding-first.json')
    assert binding['revision'] == '490d592eaa1ab02a8961044160f2b7832e1bef06'
    assert binding['receipt_revision'] == '464dcd29c8e91f78fab38f36c50437e2ae28838c'
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
    assert len(native['cases']) == len(getters['cases']) == len(read_binding['candidates']) == 38
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
    guards = verify_guards(store, pins, binding, historical_root)
    assert read_binding['guard_report'] == store.pin(GUARD_WORK + '/report.json')
    assert read_binding['guard_report'] in getters['before']
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
        after = reference.inspect(case, store.path(candidate['image']['path']), streams[0], streams[1], encoder_path=pins.check(case['encoder']), after=True)
        transition = reference.transition(case, states[case['source_id']], after)
        assert transition == outcome['transition'] and reference.describe(after) == outcome['facts']
        for placement in details['placements']:
            actual = after['table']['rows'][placement['id']]
            assert (actual['page'], actual['slot']) == (placement['page'], placement['slot'])
            assert after['table']['raw_rows'][placement['id']] == placement['record_hex']
        physical_pairs += sum(tree['entries'] for tree in details['trees'])
        results.append({'id': case['id'], 'rows': after['table']['count'], 'transition': transition,
                        'candidate': candidate['image'], 'index_allocations': details['allocations']})
    git = git_archive(store, pins, binding) if verify_git else None
    pins.terminal()
    return {'available': True, 'verified': True, 'candidate_images': 38,
            'fresh_getter_calls_replayed': 76, 'native_guard_checks': guards, 'public_reference_lineage': references,
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
