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
from ace16_insert_guid_tuple_indexes import model, reference, recipes, stderr_policy

SOURCE = '.cache/ace16-guid-tuple-index-insert-source-next-20261028'
REFERENCE_WORK = '.cache/ace16-guid-tuple-index-insert-reference-first-20261028'
EXPECTED = '.cache/ace16-guid-tuple-index-insert-expected-first-20261028'
NATIVE = '.cache/ace16-guid-tuple-index-insert-candidates-next-20261028'
NATIVE_WORK = '.cache/ace16-guid-tuple-index-insert-candidates-first-20261028'
GETTERS = '.cache/ace16-guid-tuple-index-insert-getters-next-20261028'
GETTER_WORK = '.cache/ace16-guid-tuple-index-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_guid_tuple_indexes'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/controller-freeze.json': '7be0b815e18d4d082d92b151cb99939dc151df9264cf575b50d9b9951f128b21',
    REFERENCE_WORK + '/report.json': '8f072158cd8511d112e79b1cd4717524461a1e0aa5dd307855ee0aecfdc9d3ec',
    EXPECTED + '/freeze.json': '4a2ad33136da0fdab1cbc1f090850ed339f9d9154c9aae97dd779cb7a8047848',
    NATIVE + '/protocol-freeze.json': '9aa0f55e5ef3a61a6fc12f45554087b4dbc64c9c13fa1e8b25ad844d8153581e',
    NATIVE + '/binding-first.json': '9752c5e25805b9c6a9f6a413127334c8fb6fd4dd479337017bff9355d5fa06ee',
    NATIVE_WORK + '/report.json': '15cb490339b091f28db7be1e57cbf670538fcc41a9dd67e5a56802d3ce579b06',
    GETTERS + '/controller-freeze.json': '4d08298c1a99d4b4073f34624101b73666cc2fc801183d45a8c5b8d5b60fa54e',
    GETTERS + '/binding-first.json': '4279f27b0d9484a476c56ce95e282ce7e7c716db3d58440a143781ba26cb693f',
    GETTER_WORK + '/report.json': '36eefd5494f34c927ed7be42f809af369e35b6b8d317a13abed9245f4ddf4430',
}


def definitions(text, excluded=()):
    return {node.name: ast.dump(node, include_attributes=False)
            for node in ast.parse(text).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)) and node.name not in excluded}


def maintained_identity(store, pins):
    private = pins.check(store.pin(SOURCE + '/proof.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/reference.py')).read_text()
    assert public == private.replace('ROOT = Path(__file__).resolve().parents[2]',
                                     'ROOT = Path(__file__).resolve().parents[3]').replace('import recipes\n', 'from ace16_insert_guid_tuple_indexes import recipes\n')
    assert definitions(private) == definitions(public)
    private = pins.check(store.pin(NATIVE + '/model.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/model.py')).read_text()
    assert public[public.index('def n('):] == private[private.index('def n('):]
    assert definitions(private, ('load_owner',)) == definitions(public)
    public_java = pins.check(store.pin(PUBLIC + '/GuidTupleIndexInsertReference.java'))
    executed_java = pins.check(store.pin(SOURCE + '/GuidTupleIndexInsertReference.java'))
    assert public_java.read_bytes() == executed_java.read_bytes()

    assert pins.check(store.pin(PUBLIC + '/recipes.py')).read_bytes() == pins.check(store.pin(SOURCE + '/recipes.py')).read_bytes()
    private = pins.check(store.pin(SOURCE + '/stderr_policy.py')).read_text()
    public = pins.check(store.pin(PUBLIC + '/stderr_policy.py')).read_text()
    assert public == private.replace('parents[2]', 'parents[3]').replace('import recipes\n', 'from ace16_insert_guid_tuple_indexes import recipes\n')
    assert definitions(private) == definitions(public)



def public_references(store, pins, states):
    plan = store.load(SOURCE + '/plan.json')
    report = command_report(store, pins, REFERENCE_WORK + '/report.json', 193, 16 * 1024 * 1024)
    assert len(report['sources']) == len(report['references']) == 32
    assert report['all_sources_admitted_before_append'] is True
    assert len(plan['commands']) == 193
    assert len({call['label'] for call in report['commands']}) == 193
    phases = [call['phase'] for call in plan['commands']]
    assert phases.index('reference-append') == 97
    assert phases[:97].count('compile') == 1 and phases[:97].count('generate') == 32
    assert phases[:97].count('before-read') == 64
    assert phases[97:].count('reference-append') == 32 and phases[97:].count('after-read') == 64
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
    return {'actual_children': 193, 'first_run_completed': True,
            'source_gates_before_any_append': 32, 'repeated_completed_calls': 0,
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
    assert frozen['all_thirty_two_source_gates_before_construction'] is True
    assert frozen['committed_new_producer_build_or_execution_inputs'] == 0
    maintained_identity(store, pins)
    source_plan = store.load(SOURCE + '/plan.json')
    plan = store.load(EXPECTED + '/plan.json')
    native_plan = store.load(NATIVE + '/protocol.json')
    getter_plan = store.load(GETTERS + '/protocol.json')
    assert plan['cases'] == native_plan['cases'] == getter_plan['cases']
    assert len(plan['cases']) == len(source_plan['origins']) == 32
    assert (native_plan['limits']['processes'], getter_plan['limits']['processes']) == (32, 64)
    assert len(getter_plan['commands']) == 64
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
        for identity, guid, score in recipes.requests(case):
            guid_fields = ('null', '') if guid is None else ('guid', str(UUID(bytes=guid)))
            score_fields = ('null', '') if score is None else ('int32', str(score))
            lines.append('\t'.join(('int32', str(identity), *guid_fields, *score_fields)) + '\n')
        assert pins.check(case['request']).read_text() == ''.join(lines)
        expected, details = model.derive(case, states[case['source_id']])
        certify_expected(case, states[case['source_id']], expected, details)
        assert pins.check(case['expected']).read_bytes() == expected
        assert store.load(pins.check(case['facts'])) == details
        modeled.append((expected, details))
    # All source/request image construction precedes any public AFTER replay.
    references = public_references(store, pins, states)
    native = command_report(store, pins, NATIVE_WORK + '/report.json', 32, 8 * 1024 * 1024)
    getters = command_report(store, pins, GETTER_WORK + '/report.json', 64, 16 * 1024 * 1024)
    binding = store.load(NATIVE + '/binding-first.json')
    read_binding = store.load(GETTERS + '/binding-first.json')
    assert binding['revision'] == 'dc2804326292682fed3fba74e32bca2aa0534ab2'
    assert binding['receipt_revision'] == '32341a6f3e2802e8af36c7be1edba3c437cc6fe9'
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
    assert len(native['cases']) == len(getters['cases']) == len(read_binding['candidates']) == 32
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
    return {'available': True, 'verified': True, 'candidate_images': 32,
            'fresh_getter_calls_replayed': 64, 'public_reference_lineage': references,
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
