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
from ace16_insert_decimal_indexes import model, reference, recipes, stderr_policy

SOURCE = '.cache/ace16-decimal-index-insert-source-next-20261028'
REFERENCE_WORK = '.cache/ace16-decimal-index-insert-reference-first-20261028'
EXPECTED = '.cache/ace16-decimal-index-insert-expected-first-20261028'
NATIVE = '.cache/ace16-decimal-index-insert-candidates-next-20261028'
NATIVE_WORK = '.cache/ace16-decimal-index-insert-candidates-first-20261028'
GETTERS = '.cache/ace16-decimal-index-insert-getters-next-20261028'
GETTER_WORK = '.cache/ace16-decimal-index-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_decimal_indexes'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/controller-freeze.json': '0d2aef26adf5eadc9bc5c871d7fa3436bbeb4eac6e1a75f254afbc3a1babc324',
    REFERENCE_WORK + '/report.json': 'e215a166ffa806e55aba72caedab49fd08668f6d832f51b97928db857eb1c4cc',
    EXPECTED + '/freeze.json': '3fc004c2edaa1711acb01fe43c8b421728f851455692939a56266f520d67b44f',
    NATIVE + '/protocol-freeze.json': 'c9e1caa287270e32ccc6ded79a89b1ba016a4e65ba63311c1f01cdb740a5b219',
    NATIVE + '/binding-first.json': 'caec17071f6d4d6febda16affebe43d042547efd30fa0b229fef7232503b5a76',
    NATIVE_WORK + '/report.json': 'b60170d500df1733970c74f5dbf515395b6832f7694c10372f0b6b72883daf7a',
    GETTERS + '/controller-freeze.json': '0026530e52b133521abe8f23cf1def84eedd1fcd8b6117585c9c63b62bea695e',
    GETTERS + '/binding-first.json': 'ca8c6d833e44512f53164851d896d6bc83d0fdb2a40c94e33bc3c91596ac8ca7',
    GETTER_WORK + '/report.json': 'f42f1e4ffdac8449f1d84b7ef2ab08f6e9e2066d972e68d31be93650ebbb5830',
}


def definitions(text, excluded=()):
    return {node.name: ast.dump(node, include_attributes=False)
            for node in ast.parse(text).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name not in excluded}


def maintained_identity(store, pins):
    private = pins.check(store.pin(SOURCE + '/proof.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/reference.py')).read_text()
    assert public == private.replace('ROOT = Path(__file__).resolve().parents[2]',
                                     'ROOT = Path(__file__).resolve().parents[3]').replace('import recipes\n', 'from ace16_insert_decimal_indexes import recipes\n')
    assert definitions(private) == definitions(public)
    private = pins.check(store.pin(NATIVE + '/model.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/model.py')).read_text()
    assert public[public.index('def n('):] == private[private.index('def n('):]
    assert definitions(private, ('load_owner',)) == definitions(public)
    public_java = pins.check(store.pin(PUBLIC + '/DecimalIndexInsertReference.java'))
    executed_java = pins.check(store.pin(SOURCE + '/DecimalIndexInsertReference.java'))
    assert public_java.read_bytes() == executed_java.read_bytes()

    assert pins.check(store.pin(PUBLIC + '/recipes.py')).read_bytes() == pins.check(store.pin(SOURCE + '/recipes.py')).read_bytes()
    private = pins.check(store.pin(SOURCE + '/stderr_policy.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/stderr_policy.py')).read_text()
    assert public == private.replace('parents[2]', 'parents[3]').replace('import recipes\n', 'from ace16_insert_decimal_indexes import recipes\n')
    assert definitions(private) == definitions(public)

    archive = GETTERS + '/before-static-summary-correction/'
    initial = pins.check(store.pin(archive + 'inspect_candidates.py')).read_text()
    current = pins.check(store.pin(GETTERS + '/inspect_candidates.py')).read_text()
    assert current == initial.replace("'source_expected_images': 32", "'source_expected_images': 48").replace("'future_calls': 64", "'future_calls': 96")
    old_summary = store.load(pins.check(store.pin(archive + 'static-unbound-first.json')))
    corrected = store.load(pins.check(store.pin(GETTERS + '/static-unbound-corrected.json')))
    assert old_summary == dict(verified=True, bound=False, source_expected_images=32, JVM_calls=0, future_calls=64)
    assert corrected == dict(verified=True, bound=False, source_expected_images=48, JVM_calls=0, future_calls=96)
    pins.check(store.pin(archive + 'controller-freeze.json'))



def public_references(store, pins, states):
    plan = store.load(SOURCE + '/plan.json')
    report = command_report(store, pins, REFERENCE_WORK + '/report.json', 289, 16 * 1024 * 1024)
    assert len(report['sources']) == len(report['references']) == 48
    assert report['all_sources_admitted_before_append'] is True
    assert len(plan['commands']) == 289
    assert len({call['label'] for call in report['commands']}) == 289
    phases = [call['phase'] for call in plan['commands']]
    assert phases.index('reference-append') == 145
    assert phases[:145].count('compile') == 1 and phases[:145].count('generate') == 48
    assert phases[:145].count('before-read') == 96
    assert phases[145:].count('reference-append') == 48 and phases[145:].count('after-read') == 96
    java_prefix = ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8', '-Duser.timezone=UTC',
                   '-Dcom.healthmarketscience.jackcess.dateTimeType=LOCAL_DATE_TIME']
    for call, declared in zip(report['commands'], plan['commands'], strict=True):
        assert call['argv'] == declared['argv'] and call['label'] == declared['label']
        stderr_policy.check(declared, next((item for item in plan['cases'] if item['id'] == declared.get('case', declared.get('origin'))), None), pins.check(call['stderr']).read_bytes())
        if declared['phase'] == 'compile':
            assert call['argv'][0] == 'javac'
        else:
            assert call['argv'][:5] == java_prefix
    results = []
    for case in plan['cases']:
        identity = case['id']
        recorded = report['references'][identity]
        after = reference.inspect(case, pins.check(recorded['image']),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-after-getter.stdout')),
            pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-after-maps.stdout')),
            encoder_path=pins.check(store.pin(case['encoder'])), after=True)
        transition = reference.transition(case, states[case['source_id']], after)
        assert recorded['proved'] and recorded['facts'] == reference.describe(after)
        assert transition == recorded['transition']
        results.append({'id': identity, 'transition': transition})
    return {'actual_children': 289, 'first_run_completed': True,
            'source_gates_before_any_append': 48, 'repeated_completed_calls': 0,
            'cases': results}


def certify_expected(case, before, expected, facts):
    requested = recipes.requests(case)
    assert model.requests(case) == requested
    all_rows = dict(before['rows'])
    for placement, values in zip(facts['placements'], requested, strict=True):
        page, slot = placement['page'], placement['slot']
        begin, end = reference.wire.frame(expected, page, slot)
        assert reference.decode_record(expected[begin:end], before['layout']) == values
        logical = page * 256 + slot
        assert logical not in all_rows
        all_rows[logical] = values
    definition = before['table']['definition']
    for physical, metadata in enumerate(before['physical']):
        _, _, entries = reference.tree(expected, metadata['root'], definition, case, physical == 0)
        wanted = sorted(recipes.encoded_key(values, case, physical == 0) + logical.to_bytes(4, 'big')
                        for logical, values in all_rows.items() if physical == 0 or recipes.eligible(values, case))
        assert entries == wanted



def replay(root, verify_git=False):
    store = ArtifactStore(root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in [SOURCE + '/controller-freeze.json',
                 NATIVE + '/protocol-freeze.json', GETTERS + '/controller-freeze.json']:
        pins.closure(path)
    frozen = pins.closure(EXPECTED + '/freeze.json')
    assert frozen['all_forty_eight_source_gates_before_construction'] is True
    assert frozen['committed_new_producer_build_or_execution_inputs'] == 0
    maintained_identity(store, pins)
    source_plan = store.load(SOURCE + '/plan.json')
    plan = store.load(EXPECTED + '/plan.json')
    native_plan = store.load(NATIVE + '/protocol.json')
    getter_plan = store.load(GETTERS + '/protocol.json')
    assert plan['cases'] == native_plan['cases'] == getter_plan['cases']
    assert len(plan['cases']) == len(source_plan['origins']) == 48
    assert (native_plan['limits']['processes'], getter_plan['limits']['processes']) == (48, 96)
    assert len(getter_plan['commands']) == 96
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
        assert recorded_sources[identity]['admitted'] is True
        assert reference.describe(state) == recorded_sources[identity]['facts']
        states[identity] = state
    modeled = []
    for case in plan['cases']:
        lines = []
        domains = recipes.domains(case)
        for row in recipes.requests(case):
            fields = ['int32', str(row[0])]
            for number in (1, 2):
                value = row[number]
                if value is None:
                    fields.extend(['null', ''])
                elif number in domains:
                    _, scale = domains[number]
                    fields.extend(['decimal', f'{value}:{scale}'])
                else:
                    fields.extend(['guid', str(UUID(bytes=value))])
            lines.append('\t'.join(fields) + '\n')
        assert pins.check(case['request']).read_text() == ''.join(lines)
        expected, details = model.derive(case, states[case['source_id']])
        certify_expected(case, states[case['source_id']], expected, details)
        assert pins.check(case['expected']).read_bytes() == expected
        assert store.load(pins.check(case['facts'])) == details
        modeled.append((expected, details))
    # All source/request image construction precedes any public AFTER replay.
    references = public_references(store, pins, states)
    native = command_report(store, pins, NATIVE_WORK + '/report.json', 48, 8 * 1024 * 1024)
    getters = command_report(store, pins, GETTER_WORK + '/report.json', 96, 16 * 1024 * 1024)
    binding = store.load(NATIVE + '/binding-first.json')
    read_binding = store.load(GETTERS + '/binding-first.json')
    assert binding['revision'] == '288aa53e1aa51129273d6609b2a002d3a2604dff'
    assert binding['receipt_revision'] == 'c8a4069b57c56db5745e1ebc90d8928727939870'
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
    assert len(native['cases']) == len(getters['cases']) == len(read_binding['candidates']) == 48
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
    return {'available': True, 'verified': True, 'candidate_images': 48,
            'fresh_getter_calls_replayed': 96, 'public_reference_lineage': references,
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
