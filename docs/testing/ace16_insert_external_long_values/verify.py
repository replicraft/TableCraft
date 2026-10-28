"""Replay saved SOURCE/request images and observations; no default child process."""
import argparse
import ast
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_external_long_values import evidence, model, reference, requests, source

SOURCE = '.cache/ace16-insert-external-long-value-source-next-20261028'
SOURCE_WORK = '.cache/ace16-insert-external-long-value-source-first-20261028'
EXPECTED = '.cache/ace16-insert-external-long-value-expected-first-20261028'
REFERENCE = '.cache/ace16-insert-external-long-value-reference-next-20261028'
REFERENCE_WORK = '.cache/ace16-insert-external-long-value-reference-first-20261028'
CONTINUATION = '.cache/ace16-insert-external-long-value-reference-continuation-next-20261028'
CONTINUATION_WORK = '.cache/ace16-insert-external-long-value-reference-continuation-first-20261028'
BUILD = '.cache/ace16-insert-external-long-value-isolated-build-20261028'
REJECTED = '.cache/ace16-insert-external-long-value-build-20261028'
NATIVE = '.cache/ace16-insert-external-long-value-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-insert-external-long-value-candidate-first-20261028'
GETTERS = '.cache/ace16-insert-external-long-value-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_external_long_values'
BUILD_RECEIPT = 'docs/testing/results/ace16-insert-external-long-value-isolated-example-build-2026-10-28.json'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/freeze.json': '9bc9bccf92a3a0872fba94aa30306e558677f288cc95401bb393b908e4e70ce7',
    SOURCE + '/model-freeze.json': 'c0f0ec8a00a32c0c75b234ff191c34bc9fb6adba860b5efc873a4c2b2b06f5e0',
    SOURCE + '/source-gates-final-first.json': '5b3982ca52f7a56605932e10fbeb17e00eb822d36e937b2a45e7ed0d99137a18',
    SOURCE_WORK + '/report.json': '2211a980e3230be24a8be46be1b9826e225e7f35ae0df87d87ad12f0ef078e30',
    EXPECTED + '/report.json': 'd7b0a3f86af739deb72e745e17bdf1bc46eebcbb8c74000df34af54230012f25',
    REFERENCE + '/freeze.json': 'bc29aeb3f6d216f30c67ef38dd3e1a697c9988f4cc94baacdc133eff40472013',
    REFERENCE_WORK + '/report.json': '3715ff0cc7ebc8b32d6e15052fd5af780345d104e61fd713dd464d6c7f37ff52',
    CONTINUATION + '/freeze.json': '6a63c9c400aed652f64c06da03d65c44658f53f572ca573c4399829cfabdc6ca',
    CONTINUATION_WORK + '/report.json': 'c59271de0e4a07ee0a5cdbf19ccae77335541df5d8d65af0dffc99cb77e44bed',
    BUILD + '/freeze.json': '7c0179adbca09c56718dc203d4f44349b9a24f599dbfed07c3cbaba45f511cf5',
    BUILD_RECEIPT: '924b761d0afb24d6342d9f16d1a3c48484e66919dc7753118714d2fb5ea7d115',
    NATIVE + '/freeze.json': '3352fd87e4f15662808117c113b216d81465141d83b85f2076b2fd3a82a3d9e6',
    NATIVE + '/getter-binding.json': '14eaf3d6b393ad970c059775947ebda967384cd9b32b30b52f3a27624c94d213',
    NATIVE_WORK + '/report.json': '5c738583ab7097f30cfed0d99ddeef52341e6ba17e31eb5cec92a0d7b43c8523',
    GETTERS + '/report.json': '48eb351bd4711d30144e1e37aa8d6c983f025bce065fb6fcf06780fcc153cc8d',
}


def maintained(store, pins):
    files = [
        (SOURCE + '/source.py', 'source.py'), (SOURCE + '/census.py', 'census.py'),
        (SOURCE + '/requests.py', 'requests.py'), (SOURCE + '/image_model.py', 'model.py'),
        (CONTINUATION + '/proof.py', 'reference.py'),
        (SOURCE + '/LongValueInsertFixture.java', 'LongValueInsertFixture.java'),
    ]
    for private_path, public_name in files:
        original = pins.check(store.pin(private_path)).read_text()
        expected = original
        if public_name == 'source.py':
            expected = expected.replace('parents[2]', 'parents[3]').replace(
                'import requests\nimport census\n',
                'from ace16_insert_external_long_values import requests, census\n')
        elif public_name == 'model.py':
            expected = expected.replace('import requests\nimport source\n',
                'from ace16_insert_external_long_values import requests, source\n')
        elif public_name == 'reference.py':
            expected = expected.replace('import source\nimport requests\n',
                'from ace16_insert_external_long_values import source, requests\n')
        actual = pins.check(store.pin(PUBLIC + '/' + public_name)).read_text()
        assert actual == expected, public_name
        if public_name.endswith('.py'):
            definitions = lambda text: [ast.dump(node, include_attributes=False)
                for node in ast.parse(text).body if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
            assert definitions(actual) == definitions(original)
    helper = pins.check(store.pin('docs/testing/parent_index_split_originals/BoundedFileChannel.java'))
    assert helper.read_bytes() == pins.check(store.pin(SOURCE + '/BoundedFileChannel.java')).read_bytes()


def public_references(store, pins, states, images):
    first = evidence.report(store, pins, REFERENCE_WORK + '/report.json', 2, completed=False)
    assert first['failure'] == 'AssertionError: ' and first['proofs'] == []
    continuation = evidence.report(store, pins, CONTINUATION_WORK + '/report.json', 30)
    plan = store.load(REFERENCE + '/plan.json')
    calls = first['commands'] + continuation['commands']
    assert len(calls) == len(plan['commands']) == len({call['label'] for call in calls}) == 32
    assert store.load(CONTINUATION + '/plan.json')['commands'] == plan['commands'][2:]
    for call, declared in zip(calls, plan['commands'], strict=True):
        assert call['argv'] == declared['argv'] and call['label'] == declared['label']
        assert pins.check(call['stderr']).read_bytes() == b''
    original = pins.check(store.pin(REFERENCE + '/proof.py')).read_text()
    corrected = pins.check(store.pin(CONTINUATION + '/proof.py')).read_text()
    assert corrected == original.replace('assert source.number(body, mask - 8) == 10',
                                          'assert source.number(body, mask - 4) == 10')
    facts = [('V2016-raw-single', continuation['saved_first_proof'])]
    facts += [(item['id'], item['facts']) for item in continuation['proofs']]
    assert len(facts) == 16
    assert facts[0][1] == store.load(CONTINUATION + '/saved-first-facts.json')
    public_differences = []
    for position, (identity, recorded) in enumerate(facts):
        source_id, selected = identity.rsplit('-', 1)
        output = REFERENCE_WORK + '/' + identity + '-reference.accdb'
        getter = calls[position * 2 + 1]['stdout']
        image = pins.check(store.pin(output))
        actual = reference.compare(states[source_id], image, pins.check(getter), selected)
        assert actual == recorded
        public = image.read_bytes()
        expected = images[identity]
        differing = sum(left != right for left, right in zip(public, expected)) + abs(len(public) - len(expected))
        public_differences.append(differing)
    assert public_differences == [0] * 16
    return {'initial_successful_children': 2, 'initial_proof_assembly_failed': True,
            'saved_only_footer_correction': True, 'remaining_unique_children': 30,
            'unique_public_children': 32, 'public_transition_proofs': 16,
            'repeated_completed_calls': 0, 'first_attempt_whole_phase': False,
            'observed_model_public_differing_bytes': public_differences}


def build_history(store, pins, verify_git):
    receipt = store.load(BUILD_RECEIPT)
    assert receipt['commit'] == '8374bad266ea9feebbf128c87fc2fec50ea36402'
    assert receipt['status'] == 0 and receipt['fresh_target_was_absent'] is True
    assert receipt['owning_engine_compilation_observed'] is True
    assert receipt['source_inventory_unchanged_after_build'] is True
    for key in ('artifact', 'build_driver', 'build_log', 'build_freeze',
                'superseded_shared_target_receipt', 'superseded_receipt_relocation'):
        pins.check(receipt[key])
    assert 'Compiling tablecraft-ace ' in pins.check(receipt['build_log']).read_text()
    relocation = store.load(pins.check(receipt['superseded_receipt_relocation']))
    rejected = store.load(pins.check(receipt['superseded_shared_target_receipt']))
    first = store.load(REJECTED + '/build-attempt.json')
    assert first['completed'] and first['exit_code'] == 0
    assert first['receipt']['path'] == relocation['original_path']
    assert first['receipt']['sha256'] == relocation['sha256'] == receipt['superseded_shared_target_receipt']['sha256']
    assert first['receipt']['bytes'] == relocation['bytes']
    assert rejected['artifact'] == first['artifact']
    pins.check(first['artifact'])
    assert receipt['artifact']['sha256'] != first['artifact']['sha256']
    diagnostic = store.load(REJECTED + '/freshness-diagnosis.json')
    assert diagnostic['binary_identical_to_previous_variable_artifact'] is True
    assert diagnostic['actual_native_candidate_calls'] == 0
    archive = evidence.archive(store, pins, receipt, verify_git)
    return {'shared_target_build_exit': 0, 'shared_target_result_qualified': False,
            'stale_binary_executed_as_candidate': False, 'fresh_isolated_target_build_exit': 0,
            'corrected_artifact': receipt['artifact'], 'Git': archive}


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = evidence.Pins(store)
    store.path(args.producer_report).stat()
    store.path(args.getter_report).stat()
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in (SOURCE + '/freeze.json', SOURCE + '/model-freeze.json',
                 REFERENCE + '/freeze.json', CONTINUATION + '/freeze.json',
                 BUILD + '/freeze.json', NATIVE + '/freeze.json', NATIVE + '/getter-binding.json'):
        pins.closure(path)
    maintained(store, pins)
    source_plan = store.load(SOURCE + '/plan.json')
    capture = evidence.report(store, pins, SOURCE_WORK + '/report.json', 9)
    assert capture['source_qualification'] is False
    for call, declared in zip(capture['commands'], source_plan['source_commands'], strict=True):
        assert call['label'] == declared['label'] and call['argv'] == declared['argv']
        assert pins.check(call['stderr']).read_bytes() == b''
    for item in capture['classes']:
        pins.check(item)
    gates = store.load(SOURCE + '/source-gates-final-first.json')
    assert gates['source_qualification'] is True and len(gates['sources']) == 4
    states = {case['id']: source.inspect(pins.check(store.pin(case['source'])),
                pins.check(store.pin(case['before'])), case['mode']) for case in source_plan['sources']}
    assert len(states) == 4
    for case in source_plan['sources']:
        assert states[case['id']]['raw'][20] == (5 if case['profile'] == 'V2016' else 6)
    expected_report = store.load(EXPECTED + '/report.json')
    assert expected_report['completed'] and expected_report['pin_errors'] == []
    assert expected_report['source_gates'] == 4 and expected_report['actual_child_commands'] == 0
    assert len(expected_report['images']) == 16
    images = {}
    # Derive every complete image before interpreting any public/native AFTER.
    for item in expected_report['images']:
        source_id, selected = item['id'].rsplit('-', 1)
        image, facts = model.derive(states[source_id], selected)
        assert image == pins.check(item['image']).read_bytes() and facts == item['facts']
        images[item['id']] = image
    history = build_history(store, pins, args.verify_git)
    public = public_references(store, pins, states, images)
    native = evidence.report(store, pins, args.producer_report, 16)
    getters = evidence.report(store, pins, args.getter_report, 16)
    plan = store.load(NATIVE + '/plan.json')
    assert plan['artifact'] == history['corrected_artifact']
    assert len(native['whole_images']) == len(getters['proofs']) == len(plan['cases']) == 16
    binding = store.load(NATIVE + '/getter-binding.json')
    assert binding['native_report'] == store.pin(args.producer_report)
    results = []
    for case, call, getter, image_item, recorded in zip(plan['cases'], native['commands'], getters['commands'],
            native['whole_images'], getters['proofs'], strict=True):
        identity = case['id']
        assert call['argv'] == case['native']['argv'] and getter['argv'] == case['getter']['argv']
        assert call['argv'] == [plan['artifact']['path'], case['source'], case['output'], 'Items',
                                case['requests'], '--value-bytes', '16384']
        request = pins.check(store.pin(case['requests'])).read_bytes()
        assert request == requests.tsv(case['selected'])
        assert pins.check(call['stdout']).read_bytes() == f"candidate_changed_rows={len(requests.rows(case['selected']))}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert pins.check(getter['stderr']).read_bytes() == b''
        image_path = pins.check(image_item['image'])
        assert image_item['id'] == recorded['id'] == identity
        assert image_path == store.path(case['output']) and image_path.read_bytes() == images[identity]
        facts = reference.compare(states[case['source_id']], image_path, pins.check(getter['stdout']), case['selected'])
        assert facts == recorded['facts']
        results.append({'id': identity, 'expected': case['expected'], 'facts': facts})
    pins.terminal()
    return {'verified': True, 'available': True, 'source_gates': 4,
            'source_capture_commands': 9, 'source_request_images_rederived_before_after': 16,
            'public_reference': public, 'build_history': history,
            'native_calls': 16, 'fresh_combined_getter_map_calls': 16,
            'native_and_getter_first_attempts': True,
            'pin_occurrences': pins.occurrences, 'distinct_paths': len(pins.items),
            'actual_native_JVM_Cargo_calls_by_verifier': 0, 'cases': results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default=NATIVE_WORK + '/report.json')
    parser.add_argument('--getter-report', default=GETTERS + '/report.json')
    args = parser.parse_args()
    try:
        result = replay(args)
    except FileNotFoundError as error:
        print(json.dumps({'verified': False, 'available': False,
                          'reason': 'Required private evidence is absent', 'missing': str(error.filename)}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'verified': False, 'available': True,
                          'reason': type(error).__name__ + ': ' + str(error)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
