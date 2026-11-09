"""Saved whole-image proof replay; default launches no child process."""
import argparse
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_external_long_values.evidence import Pins, report
from ace16_nullable_split_batch_updates import bootstrap, build, observations

PUBLIC = ROOT / 'docs/testing/ace16_nullable_split_batch_updates'
SOURCE = '.cache/ace16-nullable-split-batch-source-next-20261029'
SOURCE_WORK = '.cache/ace16-nullable-split-batch-source-first-20261029'
EXPECTED = '.cache/ace16-nullable-split-batch-expected-next-20261029'
EXPECTED_WORK = '.cache/ace16-nullable-split-batch-expected-first-20261029'
REFERENCE = '.cache/ace16-nullable-split-batch-reference-next-20261029'
REFERENCE_WORK = '.cache/ace16-nullable-split-batch-reference-first-20261029'
COMPILE_WORK = '.cache/ace16-nullable-split-batch-action-compile-first-20261029'
NATIVE = '.cache/ace16-nullable-split-batch-acceptance-next-20261029'
NATIVE_WORK = '.cache/ace16-nullable-split-batch-native-first-20261029'
FRESH_WORK = '.cache/ace16-nullable-split-batch-getters-first-20261029'


def normalized(value):
    return json.loads(json.dumps(value))


def commands(actual, expected, key='id'):
    assert len(actual) == len(expected)
    for saved, planned in zip(actual, expected, strict=True):
        assert saved['label'] == planned[key] and saved['argv'] == planned['argv']


def native_assignments(case, originals, request):
    table = originals['facts']['tables'][request['table']]
    roots = {row['id']: (ordinal, row) for ordinal, row in enumerate(table['rows'])}
    expected = []
    for identity, changes in request['updates']:
        ordinal, original = roots[identity]
        for column in ('Group', 'Score'):
            expected.append(dict(table=request['table'], column=column, ordinal=ordinal,
                                 identity=identity, logical_reference=original['root'],
                                 expected=original['values'][column], value=changes[column]))
    assert case['assignments'] == expected
    argv = [case['native']['argv'][0], case['source']['path'], case['output'], '--index-growth']
    for item in expected:
        encode = lambda value: 'null' if value is None else str(value)
        argv.extend(['--set', item['table'], item['column'], str(item['ordinal']),
                     'int32', encode(item['expected']), encode(item['value'])])
    assert case['native']['argv'] == argv


def all_images(store, pins, cases, expected_by_id):
    for case in cases:
        expected = expected_by_id[case['id']]['expected']
        assert case['expected'] == expected
        actual = pins.check(store.pin(case['output']))
        assert actual.read_bytes() == pins.check(expected).read_bytes(), case['id']


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    anchors = json.loads((PUBLIC / 'anchors.json').read_text())
    for path, item in anchors['phase_anchors'].items():
        # No public or candidate AFTER bytes until all reconstructions complete.
        if path.startswith(SOURCE) or path.startswith(EXPECTED):
            pins.check(item)
    source_freeze = pins.closure(SOURCE + '/source-freeze.json')
    input_freeze = pins.closure(EXPECTED + '/freeze.json')
    output_freeze = pins.closure(EXPECTED_WORK + '/freeze.json')
    assert output_freeze['completed'] and output_freeze['derivations'] == 20
    for item in output_freeze['outputs']:
        pins.check(item)
    exact_programs = bootstrap.identity(store, pins, anchors)
    source_capture = report(store, pins, SOURCE_WORK + '/report.json', 9)
    assert source_capture['source_qualification'] is False
    assert source_capture['public_reference_commands'] == 0 and len(source_capture['images']) == 4
    source_plan = store.load(SOURCE + '/source-plan.json')
    commands(source_capture['commands'], source_plan['commands'])
    gates = store.load(SOURCE + '/source-gates-first.json')
    assert gates['source_qualification'] and gates['source_gates'] == 8
    assert gates['sentinel_gates'] == 4 and gates['constructor_images'] == 0
    for item in gates['pins']:
        pins.check(item)
    expected_report = store.load(EXPECTED_WORK + '/report.json')
    assert expected_report['completed'] and expected_report['pin_errors'] == []
    assert expected_report['actual_child_commands'] == 0
    assert expected_report['source_gates_before_images'] and expected_report['model_derivations'] == 20
    requests = store.load(EXPECTED + '/requests.json')['cases']
    expected_by_id = {item['case']: item for item in expected_report['results']}
    original_states = {}
    total_bytes = 0
    dependencies = source_freeze['inputs'] + input_freeze['inputs']
    with bootstrap.load(store, pins, anchors, dependencies, native=True) as code:
        for saved in source_capture['commands']:
            stderr = pins.check(saved['stderr']).read_bytes()
            if saved['label'].endswith('-create'):
                case = next(case for case in source_plan['cases'] if saved['label'] == case['id'] + '-create')
                code.source_warnings.require_create(stderr, case['source'])
                expected_stdout = ('CREATED\t' + case['profile'] + '\t' + case['direction'] + '\n').encode()
                assert pins.check(saved['stdout']).read_bytes() == expected_stdout
            else:
                assert stderr == b''
                if saved['label'] == 'compile':
                    assert pins.check(saved['stdout']).read_bytes() == b''
        for saved in gates['facts']:
            path, getter = pins.check(saved['source']), pins.check(saved['getter'])
            facts = code.source.inspect(path, getter, saved['profile'], saved['direction'])
            assert normalized(facts) == saved['facts']
            original_states[saved['id']] = dict(raw=path.read_bytes(), facts=normalized(facts))
        assert len(original_states) == 4 and len(requests) == 20
        for request in requests:
            state = original_states[request['source_id']]
            assert request['updates'] == code.model.requests(request)
            image, details = code.model.derive(request, state, request['updates'])
            proof = code.check.certify(request, state, image, details)
            saved = expected_by_id[request['id']]
            assert image == pins.check(saved['expected']).read_bytes(), request['id']
            assert normalized(proof) == saved['proof']
            saved_details = store.load(pins.check(saved['details']))
            assert normalized(dict(details, raw_readback=proof)) == saved_details
            total_bytes += len(image)
        assert total_bytes == 9928704

        # Only now consume public AFTER, committed producer and native results.
        for item in anchors['phase_anchors'].values():
            pins.check(item)
        compiled = report(store, pins, COMPILE_WORK + '/report.json', 1)
        assert len(compiled['classes']) == 1
        assert compiled['public_reference_commands'] == compiled['native_commands'] == 0
        for item in compiled['classes']:
            pins.check(item)
        assert all(pins.check(compiled['commands'][0][stream]).read_bytes() == b''
                   for stream in ('stdout', 'stderr'))
        public_report = report(store, pins, REFERENCE_WORK + '/report.json', 40)
        public_plan = store.load(REFERENCE + '/reference-plan.json')
        commands(public_report['commands'], public_plan['commands'])
        assert len(public_report['proofs']) == len(public_report['images']) == 20
        public_observed = []
        for position, (case, saved) in enumerate(zip(public_plan['cases'], public_report['proofs'], strict=True)):
            action, getter = public_report['commands'][2 * position:2 * position + 2]
            assert pins.check(getter['stderr']).read_bytes() == b''
            facts = code.reference.compare(case, pins.check(getter['stdout']),
                                           pins.check(action['stdout']), pins.check(action['stderr']))
            assert normalized(facts) == saved['facts']
            before = code.reference.state(pins.check(store.pin(case['source'])).read_bytes(),
                                          pins.check(store.pin(case['before_getter'])).read_text())
            public_observed.append(observations.compare(
                case, before['tables'][case['table']], pins.check(saved['output']).read_bytes(),
                pins.check(expected_by_id[case['id']]['expected']).read_bytes(), facts, code.reference))
        observed = store.load(REFERENCE + '/public-observations-first.json')
        assert public_observed == observed['cases']
        assert observed['public_total_bytes'] == 10092544
        assert sum(item['warnings'] for item in public_observed) == 1600
        assert sum(item['byte_exact'] for item in public_observed) == 4
        differences = store.load('.cache/ace16-nullable-split-batch-differences-20261029/summary.json')
        pins.check(differences['comparison'])
        pins.check(differences['classifier'])
        assert differences['cases'] == 20 and differences['byte_identical_cases'] == 4
        assert differences['common_differing_bytes'] == sum(
            item['common_prefix_differing_bytes'] for item in public_observed)
        assert differences['public_only_bytes'] == sum(item['public_extra_bytes'] for item in public_observed)
        assert differences['NULL_payload_differing_bytes'] == sum(
            item['inactive_null_payload_differing_bytes'] for item in public_observed) == 11184
        assert differences['no_unexplained_typed_or_protected_prefix_differences'] is True

        receipt, build_result = build.verify(store, pins, args.verify_git)
        assert receipt['expected_outputs_frozen_before_build'] == store.pin(EXPECTED_WORK + '/freeze.json')
        protocol = pins.closure(NATIVE + '/freeze.json')
        plan = store.load(NATIVE + '/plan.json')
        assert plan['artifact'] == receipt['artifacts']['batch']
        assert plan['public_after_authority'] is False
        assert len(plan['cases']) == 20
        for case, request in zip(plan['cases'], requests, strict=True):
            assert case['id'] == request['id'] and case['request'] == request
            original = next(item for item in gates['facts'] if item['id'] == request['source_id'])
            assert case['source'] == original['source']
            assert case['before_getter'] == original['getter']
            native_assignments(case, original_states[request['source_id']], request)
        native_report = report(store, pins, NATIVE_WORK + '/report.json', 20)
        commands(native_report['commands'], [case['native'] for case in plan['cases']], key='label')
        assert native_report['phase'] == 'native'
        all_images(store, pins, plan['cases'], expected_by_id)
        assert len(native_report['whole_images']) == 20
        assert native_report['proofs'] == []
        for case, saved in zip(plan['cases'], native_report['whole_images'], strict=True):
            assert saved == dict(id=case['id'], image=store.pin(case['output']))
        for case, saved in zip(plan['cases'], native_report['commands'], strict=True):
            assert pins.check(saved['stdout']).read_bytes() == f"candidate_changed_rows={case['changed_rows']}\n".encode()
            assert pins.check(saved['stderr']).read_bytes() == (
                b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\n')
        binding = pins.closure(NATIVE + '/getter-binding.json')
        assert binding['native_report'] == store.pin(NATIVE_WORK + '/report.json')
        assert binding['phase'] == 'fresh20-after-all20-exact-engineering-images'
        fresh_report = report(store, pins, FRESH_WORK + '/report.json', 20)
        assert fresh_report['phase'] == 'getters' and len(fresh_report['proofs']) == 20
        commands(fresh_report['commands'], [case['getter'] for case in plan['cases']], key='label')
        for position, (case, saved) in enumerate(zip(plan['cases'], fresh_report['proofs'], strict=True)):
            all_images(store, pins, plan['cases'], expected_by_id)
            for item in plan['classes']:
                pins.check(item)
            getter = fresh_report['commands'][position]
            assert pins.check(getter['stderr']).read_bytes() == b''
            action = native_report['commands'][position]
            facts = code.native_proof.compare(case['proof_case'], pins.check(getter['stdout']),
                                             pins.check(action['stdout']), pins.check(action['stderr']))
            assert normalized(facts) == saved['facts']
            all_images(store, pins, plan['cases'], expected_by_id)
        pins.terminal()
    return dict(verified=True, source_commands=9, source_targets=8, sentinels=4,
                exact_maintained_programs=exact_programs, reconstructed_images=20,
                reconstructed_bytes=total_bytes, action_compile_commands=1,
                public_commands=40, public_typed_proofs=20, public_warnings=1600,
                public_byte_exact=4, native_candidates_exact=20, fresh_typed_proofs=20,
                native_Access_qualification=False, checked_pins=pins.occurrences,
                distinct_paths=len(pins.items), **build_result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true')
    args = parser.parse_args()
    try:
        result = replay(args)
    except Exception as error:
        print(json.dumps(dict(verified=False, error=type(error).__name__ + ': ' + str(error))), file=sys.stderr)
        raise SystemExit(2) from error
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
