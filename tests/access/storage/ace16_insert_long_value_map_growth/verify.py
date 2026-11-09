"""Saved column-map growth proof; no producer/Java/Cargo or default subprocess."""
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
from ace16_insert_external_long_values.evidence import Pins, report
from ace16_insert_long_value_map_growth import build, model, reference, requests, source

SOURCE = '.cache/ace16-insert-long-value-map-growth-source-next-20261028'
SOURCE_WORK = '.cache/ace16-insert-long-value-map-growth-source-first-20261028'
EXPECTED = '.cache/ace16-insert-long-value-map-growth-expected-first-20261028'
REFERENCE = '.cache/ace16-insert-long-value-map-growth-reference-next-20261028'
REFERENCE_WORK = '.cache/ace16-insert-long-value-map-growth-reference-first-20261028'
BUILD = '.cache/ace16-insert-long-value-map-growth-build-20261028'
NATIVE = '.cache/ace16-insert-long-value-map-growth-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-insert-long-value-map-growth-candidate-first-20261028'
GETTERS = '.cache/ace16-insert-long-value-map-growth-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_long_value_map_growth'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    SOURCE + '/freeze.json': 'b32ca7ab1e7d6356bd957340f4f7628c90bb085db05b5ed34e0aee6152e33de1',
    SOURCE + '/source-gates-first.json': 'd26176d3e5750bf1fad360c0111052f712d8c9598419b2457a1d44a907acb3d8',
    SOURCE + '/model-freeze.json': '3a53288ea177cd3186678e12e3765f02b808c8bbf93a67e2c7799d962823b695',
    SOURCE_WORK + '/report.json': 'a800e5cee4f8bc13e76be65f487557a5e0780e849f941946276f3a78f852ec25',
    EXPECTED + '/report.json': 'afe4b883d1fe8ef14dbe9bff93d9b755e47ef83c2e24645f218003dd826ddf79',
    REFERENCE + '/freeze.json': '759f4f59f4c0ea2112e8095f8005540cb8e92bc99de90d09f9dfee4970852991',
    REFERENCE_WORK + '/report.json': '0b9b95f6894cef06d10d83125a73f4cd240bd4d89fb077b99dbe9fdb26438ef2',
    BUILD + '/freeze.json': '3fa0c65fe6e9fa6b11fe97a54ed914d9039e7c63dec0538118fc8d1e3062b426',
    build.RECEIPT: 'b57fe38a0defbf348969ec1558285242781af0b2f74abe31ab8018cf7134322b',
    NATIVE + '/freeze.json': '8da6cb6e998faa929e50ca7070c95e14b53fc1387ea587f58aad3303cd1b3877',
    NATIVE + '/getter-binding.json': 'cf75e9bf4d1f1402056841f7ac143def7574ee327e943b9fc7cd8414d4e21e02',
    NATIVE_WORK + '/report.json': 'c273b7def8aa6ed452f25e9530f97c921e16d10dd4e1f03cc5f4db0881d00524',
    GETTERS + '/report.json': 'b47b6fe5a52c139a9eb0decc92329fce1e1d8d9d2b3745bfd8d7a9fd448810a3',
}


def maintained(store, pins):
    for owner, name in [(SOURCE, 'source.py'), (SOURCE, 'requests.py'), (SOURCE, 'model.py'),
                         (REFERENCE, 'reference.py'), (SOURCE, 'LongValueMapGrowthFixture.java')]:
        private_name = 'proof.py' if name == 'reference.py' else name
        original = pins.check(store.pin(owner + '/' + private_name)).read_text()
        expected = original
        if name in ('source.py', 'model.py'):
            expected = expected.replace('parents[2]', 'parents[3]')
        if name == 'source.py':
            expected = expected.replace('import requests\n',
                'from ace16_insert_long_value_map_growth import requests\n')
        elif name in ('model.py', 'reference.py'):
            expected = expected.replace('import source\nimport requests\n',
                'from ace16_insert_long_value_map_growth import source, requests\n')
        actual = pins.check(store.pin(PUBLIC + '/' + name)).read_text()
        assert actual == expected, name
        if name.endswith('.py'):
            definitions = lambda text: [ast.dump(node, include_attributes=False)
                for node in ast.parse(text).body if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
            assert definitions(actual) == definitions(original)
    helper = pins.check(store.pin('docs/testing/parent_index_split_originals/BoundedFileChannel.java'))
    assert helper.read_bytes() == pins.check(store.pin(SOURCE + '/BoundedFileChannel.java')).read_bytes()


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    store.path(args.producer_report).stat()
    store.path(args.getter_report).stat()
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in (SOURCE + '/freeze.json', SOURCE + '/model-freeze.json', REFERENCE + '/freeze.json',
                 BUILD + '/freeze.json', NATIVE + '/freeze.json', NATIVE + '/getter-binding.json'):
        pins.closure(path)
    maintained(store, pins)
    plan = store.load(SOURCE + '/plan.json')
    captured = report(store, pins, SOURCE_WORK + '/report.json', 17)
    assert captured['source_qualification'] is False
    assert len(captured['images']) == 8
    for actual, declared in zip(captured['commands'], plan['source_commands'], strict=True):
        assert actual['argv'] == declared['argv'] and actual['label'] == declared['label']
        assert pins.check(actual['stderr']).read_bytes() == b''
    for item in captured['classes']:
        pins.check(item)
    qualified = store.load(SOURCE + '/source-gates-first.json')
    assert qualified['source_qualification'] and len(qualified['sources']) == 8
    states = {}
    for case, recorded in zip(plan['sources'], qualified['sources'], strict=True):
        state = source.inspect(pins.check(store.pin(case['source'])), pins.check(store.pin(case['before'])),
                               case['mode'], case['padding_rows'])
        assert recorded['id'] == case['id']
        assert state['growth'] == recorded['growth']
        assert json.loads(json.dumps(state['census'])) == recorded['catalog_census']
        assert state['raw'][20] == (5 if case['profile'] == 'V2016' else 6)
        states[case['id']] = state
    assert len(states) == 8
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['pin_errors'] == [] and len(expected['images']) == 16
    assert expected['source_gates'] == 8 and expected['actual_child_commands'] == 0
    images = {}
    for item in expected['images']:
        source_id, selected = item['id'].rsplit('-', 1)
        raw, facts = model.derive(states[source_id], selected)
        assert raw == pins.check(item['image']).read_bytes() and facts == item['facts']
        images[item['id']] = raw
    # Every complete SOURCE/request image exists before inspecting any AFTER.
    receipt, git = build.verify(store, pins, args.verify_git)
    public_plan = store.load(REFERENCE + '/plan.json')
    assert public_plan['commands'] == [dict(command, phase='public-reference')
                                        for command in plan['future_reference_commands']]
    public = report(store, pins, REFERENCE_WORK + '/report.json', 32)
    assert public['source_gates'] == 8 and len(public['proofs']) == 16
    assert len({call['label'] for call in public['commands']}) == 32
    for call, declared in zip(public['commands'], public_plan['commands'], strict=True):
        assert call['argv'] == declared['argv'] and call['label'] == declared['label']
        assert pins.check(call['stderr']).read_bytes() == b''
    public_differences = []
    for position, item in enumerate(public['proofs']):
        identity = item['id']
        source_id, selected = identity.rsplit('-', 1)
        raw_path = pins.check(store.pin(REFERENCE_WORK + '/' + identity + '-reference.accdb'))
        stdout = pins.check(public['commands'][position * 2 + 1]['stdout'])
        facts = reference.compare(states[source_id], raw_path, stdout, selected)
        assert facts == item['facts']
        public_raw, expected_raw = raw_path.read_bytes(), images[identity]
        differing = sum(left != right for left, right in zip(public_raw, expected_raw))
        differing += abs(len(public_raw) - len(expected_raw))
        public_differences.append(differing)
    assert public_differences == [0] * 16
    native = report(store, pins, args.producer_report, 16)
    getters = report(store, pins, args.getter_report, 16)
    runtime = store.load(NATIVE + '/plan.json')
    binding = store.load(NATIVE + '/getter-binding.json')
    assert runtime['artifact'] == receipt['artifact']
    assert binding['native_report'] == store.pin(args.producer_report)
    assert len(native['whole_images']) == len(getters['proofs']) == len(runtime['cases']) == 16
    results = []
    for case, call, getter, image, recorded in zip(runtime['cases'], native['commands'], getters['commands'],
                                                  native['whole_images'], getters['proofs'], strict=True):
        identity = case['id']
        assert call['argv'] == case['native']['argv'] and getter['argv'] == case['getter']['argv']
        assert call['argv'] == [runtime['artifact']['path'], case['source'], case['output'], 'Items',
                                case['requests'], '--value-bytes', '16384']
        assert pins.check(store.pin(case['requests'])).read_bytes() == requests.tsv(case['selected'])
        assert pins.check(call['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert pins.check(getter['stderr']).read_bytes() == b''
        assert image['id'] == recorded['id'] == identity
        path = pins.check(image['image'])
        assert path == store.path(case['output']) and path.read_bytes() == images[identity]
        facts = reference.compare(states[case['source_id']], path, pins.check(getter['stdout']), case['selected'])
        assert facts == recorded['facts']
        results.append({'id': identity, 'expected': case['expected'], 'facts': facts})
    pins.terminal()
    return {'verified': True, 'available': True, 'source_capture_commands': 17,
            'separate_saved_source_gates': 8, 'source_request_whole_images_before_AFTER': 16,
            'public_reference_commands': 32, 'public_reference_proofs': 16,
            'native_calls': 16, 'fresh_combined_getter_map_calls': 16,
            'all_actual_phases_first_attempt': True, 'repeated_completed_calls': 0,
            'public_model_differing_bytes': public_differences,
            'build_artifact': receipt['artifact'], 'Git': git,
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
