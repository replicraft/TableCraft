"""Saved empty-owned INSERT proof; default mode starts no subprocess."""
import argparse
import ast
import csv
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_external_long_values.evidence import Pins, report
from ace16_insert_empty_owned import build, model, reference, source

SOURCE = '.cache/ace16-empty-owned-insert-source-next-20261028'
SOURCE_WORK = '.cache/ace16-empty-owned-insert-source-first-20261028'
CONSTRUCTOR = '.cache/ace16-empty-owned-insert-candidates-next-20261028'
EXPECTED = '.cache/ace16-empty-owned-insert-expected-first-20261028'
REFERENCE = '.cache/ace16-empty-owned-insert-public-next-20261028'
REFERENCE_WORK = '.cache/ace16-empty-owned-insert-public-first-20261028'
BUILD = '.cache/ace16-insert-empty-owned-isolated-build-20261028'
NATIVE = '.cache/ace16-empty-owned-insert-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-empty-owned-insert-candidate-first-20261028'
GETTERS = '.cache/ace16-empty-owned-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_empty_owned'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'

FIXED = {
    '.cache/ace16-empty-owned-insert-source-next-20261028/plan-helper-freeze.json': (
        '4daa0e7f5c188c4591dc757778246d77a3636fabe6252e423e10df5cbd07eb6c'
    ),
    '.cache/ace16-empty-owned-insert-source-next-20261028/source-freeze.json': (
        '7efef7116b011576de7cf9b8cab9ed3fe291b02e5f2e0a80bc2d459fdf9e9655'
    ),
    '.cache/ace16-empty-owned-insert-source-next-20261028/source-gates-first.json': (
        'a239b157ab30e4bf6456f76458bce98826fdcdd47e82b7183f9e01cf8965bdb5'
    ),
    '.cache/ace16-empty-owned-insert-source-first-20261028/report.json': (
        '40253335b1a3cb15049b82fb939bfa5853985b7f39134c6f43a33594922d00f8'
    ),
    '.cache/ace16-empty-owned-insert-candidates-next-20261028/model-freeze.json': (
        'bba07439ad6f4d036d3ce3a7715420835e7949e6c0d0ef088bb73cb0d241f192'
    ),
    '.cache/ace16-empty-owned-insert-candidates-next-20261028/model-freeze-v2.json': (
        '0febe7f9b0ed162ce0b6fdad8a62004bbfd43d90d98d8d77d9b14eed1b58e1c7'
    ),
    '.cache/ace16-empty-owned-insert-expected-first-20261028/freeze.json': (
        '14e90f8d175534352b91eead34dcb94d3987c48ee74e29afa75720bca6d8d331'
    ),
    '.cache/ace16-empty-owned-insert-expected-first-20261028/report.json': (
        '2c39572df927b435080a860511d2ac3809c152495864e9c52d32c472a06b0996'
    ),
    '.cache/ace16-empty-owned-insert-public-next-20261028/freeze.json': (
        'b2eb96bce5067a2e0067628629b7eb7392676645ffcbec50725c189ab9d33119'
    ),
    '.cache/ace16-empty-owned-insert-public-first-20261028/report.json': (
        '30d72383cdda4a0003d4363711302077d65df3c7bd976d43be4b875b4ae48cb7'
    ),
    '.cache/ace16-insert-empty-owned-isolated-build-20261028/freeze.json': (
        'eb67a5c8b73ba499ece0295bf5a5a29badfc7bf6dc3d46754db819f00960a2e7'
    ),
    'docs/testing/results/ace16-insert-empty-owned-example-build-2026-10-28.json': (
        'aed2dea58e362658b49c817b3b89acd174102a777ac09c10f8fda142f34a8515'
    ),
    '.cache/ace16-empty-owned-insert-acceptance-next-20261028/freeze.json': (
        '55034fde7e0de641ba80725410fa52550fd6a4814f784a409fc1c6a338aff050'
    ),
    '.cache/ace16-empty-owned-insert-acceptance-next-20261028/getter-binding.json': (
        '5161c5e37349b2562efd3673701b584943c7dbc708f4342c603a91a4d814d505'
    ),
    '.cache/ace16-empty-owned-insert-candidate-first-20261028/report.json': (
        'a753fadaee74c05ab25c366f45387be665676dcd40e48434033bd0a709a928a2'
    ),
    '.cache/ace16-empty-owned-insert-getters-first-20261028/report.json': (
        'bdd40da34ee474f8d4a8a4b384690f226e409537b20b4e7e67beedfb79da7612'
    ),
}


def maintained(store, pins):
    owners = [(SOURCE, 'source.py', 'source.py'),
              (CONSTRUCTOR, 'model.py', 'model.py'),
              (REFERENCE, 'proof.py', 'reference.py'),
              (SOURCE, 'plan.json', 'plan.json'),
              (SOURCE, 'EmptyOwnedInsertFixture.java', 'EmptyOwnedInsertFixture.java'),
              (CONSTRUCTOR, 'MODEL_POLICY.md', 'MODEL_POLICY.md')]
    for owner, original_name, name in owners:
        original = pins.check(store.pin(owner + '/' + original_name)).read_text()
        expected = original
        if name in ('source.py', 'model.py'):
            expected = expected.replace('parents[2]', 'parents[3]')
        if name == 'reference.py':
            expected = expected.replace('import source\n',
                                        'from ace16_insert_empty_owned import source\n')
        actual = pins.check(store.pin(PUBLIC + '/' + name)).read_text()
        assert actual == expected, name
        if name.endswith('.py'):
            def functions(text):
                return [ast.dump(node, include_attributes=False)
                        for node in ast.parse(text).body
                        if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
            assert functions(actual) == functions(original)
    for selected in ('single', 'batch'):
        name = '/requests/' + selected + '.tsv'
        assert pins.check(store.pin(PUBLIC + name)).read_bytes() == pins.check(store.pin(SOURCE + name)).read_bytes()
    assert pins.check(store.pin(SOURCE + '/BoundedFileChannel.java')).read_bytes() == pins.check(
        store.pin('docs/testing/parent_index_split_originals/BoundedFileChannel.java')).read_bytes()


def preparation_history(store, pins):
    before = CONSTRUCTOR + '/before-census-json-normalization'
    prior = pins.check(store.pin(before + '/prepare.py')).read_text()
    current = pins.check(store.pin(CONSTRUCTOR + '/prepare.py')).read_text()
    assert prior.replace("assert state['census'] == gate['facts']['census']",
                         "assert json.loads(json.dumps(state['census'])) == gate['facts']['census']") == current
    assert pins.check(store.pin(before + '/model.py')).read_bytes() == pins.check(
        store.pin(CONSTRUCTOR + '/model.py')).read_bytes()
    assert pins.check(store.pin(before + '/model-freeze.json')).read_bytes() == pins.check(
        store.pin(CONSTRUCTOR + '/model-freeze.json')).read_bytes()
    note = pins.check(store.pin(before + '/first-static-failure.txt')).read_text()
    assert 'No expected images were derived' in note
    public_note = pins.check(store.pin(REFERENCE + '/before-raw-owner-freeze-guard/first-static-stop.txt')).read_text()
    assert 'No public output/command/proof ran' in public_note


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in (SOURCE + '/plan-helper-freeze.json', SOURCE + '/source-freeze.json',
                 CONSTRUCTOR + '/model-freeze-v2.json', EXPECTED + '/freeze.json',
                 REFERENCE + '/freeze.json', BUILD + '/freeze.json',
                 NATIVE + '/freeze.json', NATIVE + '/getter-binding.json'):
        pins.closure(path)
    maintained(store, pins)
    preparation_history(store, pins)
    plan = store.load(SOURCE + '/plan.json')
    captured = report(store, pins, SOURCE_WORK + '/report.json', 3)
    assert captured['source_qualification'] is False and len(captured['images']) == 1
    for call, expected in zip(captured['commands'], plan['source_commands'], strict=True):
        assert call['argv'] == expected['argv'] and call['label'] == expected['label']
        assert pins.check(call['stderr']).read_bytes() == b''
    for item in captured['classes']:
        pins.check(item)
    gate = store.load(SOURCE + '/source-gates-first.json')
    assert gate['source_qualification'] and gate['source_gates'] == 1
    assert gate['actual_child_commands'] == gate['expected_images_computed'] == 0
    origin = plan['origin']
    state = source.inspect(pins.check(gate['source']), pins.check(gate['getter']))
    assert state['source_owned_record'] == gate['facts']['owned_record']
    assert json.loads(json.dumps(state['census'])) == gate['facts']['census']
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['pin_errors'] == []
    assert expected['source_gates'] == 1 and expected['actual_child_commands'] == 0
    assert len(expected['images']) == 2
    images = {}
    for item, selected in zip(expected['images'], ('single', 'batch'), strict=True):
        assert item['selected'] == selected and item['source'] == gate['source']
        requests = origin['requests'][selected]
        tsv = pins.check(store.pin(SOURCE + '/requests/' + selected + '.tsv')).read_text()
        assert list(csv.reader(tsv.splitlines(), delimiter='\t')) == [
            ['int32', str(identity), 'binary', binary] for identity, binary in requests]
        raw, facts = model.derive(state, requests)
        assert raw == pins.check(item['image']).read_bytes() and facts == item['facts']
        images[item['id']] = raw
    # Both complete images are reconstructed before any AFTER is interpreted.
    receipt, git = build.verify(store, pins, args.verify_git)
    public_plan = store.load(REFERENCE + '/plan.json')
    assert public_plan['commands'] == plan['future_reference_commands']
    public = report(store, pins, REFERENCE_WORK + '/report.json', 4)
    assert public['source_gates'] == 1 and len(public['proofs']) == 2
    for call, expected_call in zip(public['commands'], public_plan['commands'], strict=True):
        assert call['argv'] == expected_call['argv'] and call['label'] == expected_call['label']
        assert pins.check(call['stderr']).read_bytes() == b''
    public_differences = []
    for position, (selected, item) in enumerate(zip(('single', 'batch'), public['proofs'], strict=True)):
        identity = origin['id'] + '-' + selected
        assert item['id'] == identity
        path = pins.check(store.pin(public_plan['outputs'][selected]))
        getter = pins.check(public['commands'][position * 2 + 1]['stdout'])
        facts = reference.compare(state, path, getter, origin['requests'][selected])
        assert facts == item['facts']
        actual, wanted = path.read_bytes(), images[identity]
        differences = sum(left != right for left, right in zip(actual, wanted))
        differences += abs(len(actual) - len(wanted))
        public_differences.append(differences)
    assert public_differences == [0, 0]
    native_plan = store.load(NATIVE + '/plan.json')
    assert native_plan['artifact'] == receipt['artifact']
    native = report(store, pins, NATIVE_WORK + '/report.json', 2)
    assert native['phase'] == 'native' and len(native['whole_images']) == 2
    # Validate both complete native outputs before reading any fresh getter.
    for case, image, call in zip(native_plan['cases'], native['whole_images'], native['commands'], strict=True):
        assert image['id'] == case['id']
        path = pins.check(image['image'])
        assert path == store.path(case['output']) and path.read_bytes() == images[case['id']]
        assert call['argv'] == case['native']['argv'] == [receipt['artifact']['path'],
            origin['source'], case['output'], 'Items', case['requests']]
        assert pins.check(call['stdout']).read_bytes() == f"candidate_changed_rows={len(case['logical_requests'])}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
    binding = store.load(NATIVE + '/getter-binding.json')
    assert binding['native_report'] == store.pin(NATIVE_WORK + '/report.json')
    fresh = report(store, pins, GETTERS + '/report.json', 2)
    assert fresh['phase'] == 'getters' and len(fresh['proofs']) == 2
    facts = []
    for case, call, recorded in zip(native_plan['cases'], fresh['commands'], fresh['proofs'], strict=True):
        assert call['argv'] == case['getter']['argv'] == native_plan['java_prefix'] + ['inspect', case['output']]
        assert pins.check(call['stderr']).read_bytes() == b''
        actual = reference.compare(state, store.path(case['output']), pins.check(call['stdout']),
                                   origin['requests'][case['selected']])
        assert recorded['id'] == case['id'] and recorded['facts'] == actual
        facts.append(dict(id=case['id'], facts=actual))
    pins.terminal()
    return dict(verified=True, available=True, source_capture_commands=3,
                saved_source_gates=1, source_request_complete_images_before_AFTER=2,
                public_reference_commands=4, public_transition_proofs=2,
                public_model_differing_bytes=public_differences,
                native_calls=2, fresh_inert_inspections=2, final_user_rows=19,
                selected_physical_key_ROW_pairs=17, old_Items_ROW_count=0,
                preparation_stops_preserved=2, runtime_phases_first_attempt=True,
                actual_native_JVM_Cargo_calls_by_verifier=0,
                pin_occurrences=pins.occurrences, distinct_paths=len(pins.items),
                build_artifact=receipt['artifact'], Git=git, cases=facts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true')
    args = parser.parse_args()
    try:
        result = replay(args)
    except FileNotFoundError as error:
        print(json.dumps(dict(verified=False, available=False,
                              reason='Required private evidence is absent', missing=str(error.filename)), indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps(dict(verified=False, available=True,
                              reason=type(error).__name__ + ': ' + str(error)), indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
