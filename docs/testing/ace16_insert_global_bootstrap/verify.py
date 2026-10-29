"""Saved GLOBAL bootstrap proof; no producer/Java/Cargo or default subprocess."""
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
from ace16_insert_global_bootstrap import build, model, reference, requests, source

SOURCE = '.cache/ace16-insert-global-bootstrap-source-next-20261028'
SOURCE_WORK = '.cache/ace16-insert-global-bootstrap-source-first-20261028'
EXPECTED = '.cache/ace16-insert-global-bootstrap-expected-first-20261028'
REFERENCE = '.cache/ace16-insert-global-bootstrap-reference-next-20261028'
REFERENCE_WORK = '.cache/ace16-insert-global-bootstrap-reference-first-20261028'
BUILD = '.cache/ace16-insert-global-bootstrap-build-20261028'
NATIVE = '.cache/ace16-insert-global-bootstrap-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-insert-global-bootstrap-candidate-first-20261028'
GETTERS = '.cache/ace16-insert-global-bootstrap-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_global_bootstrap'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {'.cache/ace16-insert-global-bootstrap-source-next-20261028/freeze.json': '3b7c602c5a8eeb1d3537b8ab898be11e8322c97e1bf2efa8211d9c858432389c',
 '.cache/ace16-insert-global-bootstrap-source-next-20261028/source-gates-first.json': 'f8d6a9ff0db462147eb57f4945d90c1125cf7e183ff4e035306f47ed9a5d31a6',
 '.cache/ace16-insert-global-bootstrap-source-next-20261028/model-freeze.json': '6637aa1dc9092459ac776903761dc451c62154fc6f734aa5fd83f864dbcc652c',
 '.cache/ace16-insert-global-bootstrap-source-first-20261028/report.json': '8121e1afa2a041c012d09a28e81f772eb14945cf73978b45e553a58d30454448',
 '.cache/ace16-insert-global-bootstrap-expected-first-20261028/report.json': '427bea6c8bbf6d5fb3ae1d82ba752196525afed14f632489e7d234f3dc07bd2f',
 '.cache/ace16-insert-global-bootstrap-reference-next-20261028/freeze.json': '0cfe3ac6e586aae8da8d7d52409b2b0c668ea9c69dda6fd63f814d7ef3e0bbc6',
 '.cache/ace16-insert-global-bootstrap-reference-first-20261028/report.json': '209e82800823800eb42758db07a95c01ee2e92ec91d738e44be59bc72375db4b',
 '.cache/ace16-insert-global-bootstrap-build-20261028/freeze.json': '1763536bb45a7998af3646cfb8a5ab5af5c18c20ada7730339c40ab4d5d3b1be',
 'docs/testing/results/ace16-insert-global-bootstrap-example-build-2026-10-28.json': '79fda89c9fa6d3134a8a46bd3a8f4f6ff4bc7fe7b5585ee7289c0993317ea6b5',
 '.cache/ace16-insert-global-bootstrap-acceptance-next-20261028/freeze.json': 'b1649465e3222f47ef7acf523d7e6d59c4a947ad164cde7c588ae9c5b2cadb45',
 '.cache/ace16-insert-global-bootstrap-acceptance-next-20261028/getter-binding.json': 'f8b53f70f65bc12cb3770b463d2e0dec3182d956a81e5432c1b1896bf6bc40b3',
 '.cache/ace16-insert-global-bootstrap-candidate-first-20261028/report.json': '4ab5340716270c87f3bac576aa566b29d5d756555942745716f9375e8b8cb34b',
 '.cache/ace16-insert-global-bootstrap-getters-first-20261028/report.json': '75a80a2b1b82addabe4cb517ba94b15bb8577bf4dc68395410baf15f143fe3a6',
 '.cache/ace16-insert-global-bootstrap-source-next-20261028/MODEL_POLICY.md': 'ef01e418e9c08f3bc2d66b7b8f01f1f16318d0b526df816f6b9b9eb14d4636af',
 '.cache/ace16-global-bootstrap-source-review-20261028/facts.json': '73b5f2b5e48d931fd2a630962e87f3d98a3a934ebcae38fabc69dafb01a3d959'}


def maintained(store, pins):
    for owner, name in [(SOURCE, 'source.py'), (SOURCE, 'requests.py'), (SOURCE, 'model.py'),
                         (REFERENCE, 'reference.py'), (SOURCE, 'GlobalBootstrapFixture.java')]:
        private_name = 'proof.py' if name == 'reference.py' else name
        original = pins.check(store.pin(owner + '/' + private_name)).read_text()
        expected = original
        if name in ('source.py', 'model.py'):
            expected = expected.replace('parents[2]', 'parents[3]')
        if name == 'source.py':
            expected = expected.replace('import requests\n',
                'from ace16_insert_global_bootstrap import requests\n')
        elif name in ('model.py', 'reference.py'):
            expected = expected.replace('import source\nimport requests\n',
                'from ace16_insert_global_bootstrap import source, requests\n')
        actual = pins.check(store.pin(PUBLIC + '/' + name)).read_text()
        assert actual == expected, name
        if name.endswith('.py'):
            definitions = lambda text: [ast.dump(node, include_attributes=False)
                for node in ast.parse(text).body if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
            assert definitions(actual) == definitions(original)
    assert pins.check(store.pin(PUBLIC + '/MODEL_POLICY.md')).read_bytes() == pins.check(store.pin(SOURCE + '/MODEL_POLICY.md')).read_bytes()
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
    primary = store.load('.cache/ace16-global-bootstrap-source-review-20261028/facts.json')
    for value in primary.values():
        if isinstance(value, dict) and set(('path', 'bytes', 'sha256')) <= value.keys():
            pins.check(value)
    for implementation in primary['implementation_files'].values():
        pins.check(implementation['artifact'])
    maintained(store, pins)
    plan = store.load(SOURCE + '/plan.json')
    captured = report(store, pins, SOURCE_WORK + '/report.json', 9)
    assert captured['source_qualification'] is False
    assert len(captured['images']) == 4
    for actual, declared in zip(captured['commands'], plan['source_commands'], strict=True):
        assert actual['argv'] == declared['argv'] and actual['label'] == declared['label']
        assert pins.check(actual['stderr']).read_bytes() == b''
    for item in captured['classes']:
        pins.check(item)
    qualified = store.load(SOURCE + '/source-gates-first.json')
    assert qualified['source_qualification'] and len(qualified['sources']) == 4
    states = {}
    for case, recorded in zip(plan['sources'], qualified['sources'], strict=True):
        state = source.inspect(pins.check(store.pin(case['source'])), pins.check(store.pin(case['before'])),
                               case['mode'], case['padding_rows'])
        assert recorded['id'] == case['id']
        assert state['bootstrap'] == recorded['bootstrap']
        assert json.loads(json.dumps(state['census'])) == recorded['catalog_census']
        assert state['raw'][20] == (5 if case['profile'] == 'V2016' else 6)
        states[case['id']] = state
    assert len(states) == 4
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['pin_errors'] == [] and len(expected['images']) == 12
    assert expected['source_gates'] == 4 and expected['actual_child_commands'] == 0
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
    public = report(store, pins, REFERENCE_WORK + '/report.json', 24)
    assert public['source_gates'] == 4 and len(public['proofs']) == 12
    assert len({call['label'] for call in public['commands']}) == 24
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
    assert public_differences == [0] * 12
    native = report(store, pins, args.producer_report, 12)
    getters = report(store, pins, args.getter_report, 12)
    runtime = store.load(NATIVE + '/plan.json')
    binding = store.load(NATIVE + '/getter-binding.json')
    assert runtime['artifact'] == receipt['artifact']
    assert binding['native_report'] == store.pin(args.producer_report)
    assert len(native['whole_images']) == len(getters['proofs']) == len(runtime['cases']) == 12
    results = []
    for case, call, getter, image, recorded in zip(runtime['cases'], native['commands'], getters['commands'],
                                                  native['whole_images'], getters['proofs'], strict=True):
        identity = case['id']
        assert call['argv'] == case['native']['argv'] and getter['argv'] == case['getter']['argv']
        assert call['argv'] == [runtime['artifact']['path'], case['source'], case['output'], 'Items',
                                case['requests'], '--value-bytes', '16384']
        assert pins.check(store.pin(case['requests'])).read_bytes() == requests.tsv(case['selected'])
        assert pins.check(call['stdout']).read_bytes() == f"candidate_changed_rows={len(requests.rows(case['selected']))}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert pins.check(getter['stderr']).read_bytes() == b''
        assert image['id'] == recorded['id'] == identity
        path = pins.check(image['image'])
        assert path == store.path(case['output']) and path.read_bytes() == images[identity]
        facts = reference.compare(states[case['source_id']], path, pins.check(getter['stdout']), case['selected'])
        assert facts == recorded['facts']
        results.append({'id': identity, 'expected': case['expected'], 'facts': facts})
    pins.terminal()
    return {'verified': True, 'available': True, 'source_capture_commands': 9,
            'separate_saved_source_gates': 4, 'source_request_whole_images_before_AFTER': 12,
            'public_reference_commands': 24, 'public_reference_proofs': 12,
            'native_calls': 12, 'fresh_combined_getter_map_calls': 12,
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
