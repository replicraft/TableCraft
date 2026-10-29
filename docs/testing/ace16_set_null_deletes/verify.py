"""Saved DELETE SET NULL proof; default replay starts no child process or writes."""
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
from ace16_set_null_deletes import build, model, reference, source

SOURCE = '.cache/ace16-set-null-delete-source-next-20261028'
SOURCE_WORK = '.cache/ace16-set-null-delete-source-first-20261028'
SAVED = '.cache/ace16-set-null-delete-saved-v2-next-20261028'
FIRST_MODEL = '.cache/ace16-set-null-delete-candidates-next-20261028'
CONSTRUCTOR = '.cache/ace16-set-null-delete-candidates-v2-next-20261028'
FIRST_EXPECTED = '.cache/ace16-set-null-delete-expected-first-20261028'
EXPECTED = '.cache/ace16-set-null-delete-expected-second-20261028'
EXPECTED_READ = '.cache/ace16-set-null-delete-expected-read-next-20261028'
EXPECTED_READ_WORK = '.cache/ace16-set-null-delete-expected-read-first-20261028'
REFERENCE = '.cache/ace16-set-null-delete-public-next-20261028'
REFERENCE_WORK = '.cache/ace16-set-null-delete-public-first-20261028'
BUILD = '.cache/ace16-set-null-delete-isolated-build-20261028'
NATIVE = '.cache/ace16-set-null-delete-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-set-null-delete-candidate-first-20261028'
GETTERS = '.cache/ace16-set-null-delete-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_set_null_deletes'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    '.cache/ace16-set-null-delete-source-next-20261028/plan-helper-freeze.json': (
        'ae24e110d984bdcc975ae11bfd839ea6c46b961ea94617548753e0161c738ed8'
    ),
    '.cache/ace16-set-null-delete-source-next-20261028/source-freeze.json': (
        'b9fe90eca51aac3e28d5bd84e7416de686eccc0ed2db049f2647dc847873a73b'
    ),
    '.cache/ace16-set-null-delete-source-first-20261028/report.json': (
        '5693f7abb5374368c7a06bb00adc3fb06195e4f7a81897e8082be810e31ef910'
    ),
    '.cache/ace16-set-null-delete-saved-v2-next-20261028/gate-freeze.json': (
        '8d6e5dc918e652f995dbd01c6f19ca8a02cbe029d0e4ef4e1d248e7228b9bc69'
    ),
    '.cache/ace16-set-null-delete-saved-v2-next-20261028/source-gates-first.json': (
        'f5b52ed8b9ec52a52455628a5d8bd5e48e3b39490766cb078a1bd33e04fa368f'
    ),
    '.cache/ace16-set-null-delete-candidates-next-20261028/model-freeze.json': (
        '1450ccfabd32442b2f7bc392a2691715b62a4d0f2907d28b581006debe0e9b98'
    ),
    '.cache/ace16-set-null-delete-candidates-v2-next-20261028/model-freeze.json': (
        '5c87c7ca1a8e415a28858285852d8b3faa82b4d3677ff752dcd2d37126603284'
    ),
    '.cache/ace16-set-null-delete-candidates-v2-next-20261028/reverify-first.json': (
        'c985d51e6b7c497d13d0d42fdfa9f06e01081b629c8cfc6e7755d13010491fc3'
    ),
    '.cache/ace16-set-null-delete-expected-second-20261028/freeze.json': (
        '3b1c02603984ceb56c2313b446d24ad5ba931dafa7b36cdc7a3d2864a219e1f4'
    ),
    '.cache/ace16-set-null-delete-expected-second-20261028/report.json': (
        'd0d03861dc6d6c93ce1802530344d4a5f43344138d8cfdf93f80b8d2071cf730'
    ),
    '.cache/ace16-set-null-delete-expected-read-next-20261028/freeze.json': (
        '90baa65d3d25a1ce7c2eacc387abd74da8b31719bf9e0dda7da7bd7e338135fb'
    ),
    '.cache/ace16-set-null-delete-expected-read-first-20261028/report.json': (
        '7bdb8bae60ca78b43865466e4304756ddd0b77c46a51d113b9fc182567008a53'
    ),
    '.cache/ace16-set-null-delete-public-next-20261028/freeze.json': (
        'c987fa9f80c18dcb3200a7d110e01ea2dab5eeca015ef8cfa3449e3e75705e42'
    ),
    '.cache/ace16-set-null-delete-public-next-20261028/public-differences-first.json': (
        '575ee5df19721976192783919cfe7bb64746389f220842e4e9f7428fddb73348'
    ),
    '.cache/ace16-set-null-delete-public-first-20261028/report.json': (
        '3fa6aaa1d48f135228f3a778b0ca675f5540d8dba59161f812fa86adf3da5292'
    ),
    '.cache/ace16-set-null-delete-isolated-build-20261028/freeze.json': (
        '0b60f9badb95e56ffe7b63eda52e2aadf12a941f197303f6f666e94b0554291f'
    ),
    'docs/testing/results/ace16-set-null-delete-example-build-2026-10-28.json': (
        '89f0bdc24961a0d49c95862b1e2ad10625cc180fa2b1d164a61fe56b3b8db4c4'
    ),
    '.cache/ace16-set-null-delete-acceptance-next-20261028/seed-freeze.json': (
        '530b1f881b04342d7abc109543132dec23b41f3ab6dcc4f82501317a4b353286'
    ),
    '.cache/ace16-set-null-delete-acceptance-next-20261028/freeze.json': (
        '38025096c492a39f935b3d5078cabfe2b7ecb1825b191bfe688c66c3712cb5ce'
    ),
    '.cache/ace16-set-null-delete-acceptance-next-20261028/getter-binding.json': (
        '5e2574a2d58ec7ebb0a762b6108607cd1275ed13c74bf5c9b4f8d1cfab3be470'
    ),
    '.cache/ace16-set-null-delete-candidate-first-20261028/report.json': (
        '86c5046b990273563d05f47d7a600ec060e35a39377f482cc844b1410b59e03d'
    ),
    '.cache/ace16-set-null-delete-getters-first-20261028/report.json': (
        '630b4b664f33876a8442a40a8f6c76b5492f3aff96b64fa7d75f8974fda2cd89'
    ),
}


def normalized(value):
    return json.loads(json.dumps(value))


def closure(store, pins, path):
    value = pins.closure(path)
    for item in value.get('outputs', []):
        if 'path' in item:
            pins.check(item)
    return value


def functions(text):
    return [ast.dump(node, include_attributes=False) for node in ast.parse(text).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef))]


def maintained(store, pins):
    owners = [
        (SAVED + '/source.py', 'source.py'),
        (CONSTRUCTOR + '/model_v2.py', 'model.py'),
        (REFERENCE + '/proof.py', 'reference.py'),
        (CONSTRUCTOR + '/MODEL_POLICY.md', 'MODEL_POLICY.md'),
        (CONSTRUCTOR + '/requests.json', 'requests.json'),
        (SOURCE + '/plan.json', 'plan.json'),
        (SOURCE + '/SetNullDeleteFixture.java', 'SetNullDeleteFixture.java'),
        (NATIVE + '/requests/basic.tsv', 'requests/basic.tsv'),
        (NATIVE + '/requests/mixed.tsv', 'requests/mixed.tsv'),
    ]
    for original_path, name in owners:
        original = pins.check(store.pin(original_path)).read_text()
        expected = original
        if name == 'source.py':
            expected = expected.replace('parents[2]', 'parents[3]')
        elif name == 'reference.py':
            expected = expected.replace('import source\n', 'from ace16_set_null_deletes import source\n')
        current = pins.check(store.pin(PUBLIC + '/' + name)).read_text()
        assert current == expected, name
        if name.endswith('.py'):
            assert functions(current) == functions(original), name
    assert pins.check(store.pin(SOURCE + '/BoundedFileChannel.java')).read_bytes() == pins.check(
        store.pin('docs/testing/parent_index_split_originals/BoundedFileChannel.java')).read_bytes()


def preparation_history(store, pins):
    original = pins.check(store.pin(SOURCE + '/source.py')).read_text()
    current = pins.check(store.pin(SAVED + '/source.py')).read_text()
    expected = original.replace("maps[owner + suffix]['reference']", "map_reference(maps[owner + suffix])")
    expected = expected.replace("maps[owner + ':IndexData:' + str(position)]['reference']",
                                "map_reference(maps[owner + ':IndexData:' + str(position)])")
    helper = '''
def map_reference(metadata):
    page = metadata['map_page']
    slot = metadata['map_slot']
    assert 0 < page < (1 << 24) and 0 <= slot < 256
    return page << 8 | slot

'''
    position = expected.index('\ndef index_declarations(')
    assert expected[:position] + helper + expected[position:] == current
    note = pins.check(store.pin(SAVED + '/CORRECTION.md')).read_text()
    assert 'KeyError reference' in note and 'No expected image' in note
    old = pins.check(store.pin(FIRST_MODEL + '/model.py')).read_text()
    new = pins.check(store.pin(CONSTRUCTOR + '/model_v2.py')).read_text()
    loop = "    for name in touched:\n        for identity, row in tables[name]['rows'].items():"
    assert old.count(loop) == 1
    assert old.replace(loop, "    for name in touched:\n        table = tables[name]\n        for identity, row in tables[name]['rows'].items():") == new
    note = pins.check(store.pin(CONSTRUCTOR + '/CORRECTION.md')).read_text()
    assert 'KeyError ParentID' in note and 'before writing any complete image' in note
    first = store.path(FIRST_EXPECTED)
    assert sorted(path.name for path in first.iterdir()) == ['before-derive-inputs.json']
    for item in store.load(pins.check(store.pin(FIRST_EXPECTED + '/before-derive-inputs.json'))):
        pins.check(item)


def compare_recorded(store, pins, state, case, path, getter, recorded):
    original_getter = store.path(SOURCE_WORK + '/' + case['selected'] + '-before-inspect.stdout')
    facts = reference.compare(state, original_getter, path, getter, case['requests'])
    assert facts == recorded
    return facts


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in (
        SOURCE + '/plan-helper-freeze.json', SOURCE + '/source-freeze.json',
        SAVED + '/gate-freeze.json', FIRST_MODEL + '/model-freeze.json',
        CONSTRUCTOR + '/model-freeze.json', EXPECTED + '/freeze.json',
        EXPECTED_READ + '/freeze.json', REFERENCE + '/freeze.json',
        BUILD + '/freeze.json', NATIVE + '/seed-freeze.json',
        NATIVE + '/freeze.json', NATIVE + '/getter-binding.json',
    ):
        closure(store, pins, path)
    maintained(store, pins)
    preparation_history(store, pins)
    original = store.load(SOURCE + '/plan.json')
    execution = store.load(SOURCE + '/execution-plan.json')
    refined = normalized(original)
    refined['commands'][0]['argv'][1:1] = ['-J-Xmx512m', '-encoding', 'UTF-8']
    refined['refinement'] = execution['refinement']
    assert execution == refined
    captured = report(store, pins, SOURCE_WORK + '/report.json', 5)
    assert captured['source_qualification'] is False and len(captured['images']) == 2
    for call, expected in zip(captured['commands'], execution['commands'], strict=True):
        assert call['argv'] == expected['argv'] and call['label'] == expected['id']
        assert pins.check(call['stderr']).read_bytes() == b''
    for item in captured['classes']:
        pins.check(item)
    gate = store.load(SAVED + '/source-gates-first.json')
    assert gate['source_qualification'] and gate['source_gates'] == 2
    assert gate['actual_child_commands'] == gate['constructor_images'] == 0
    for item in gate['pins']:
        pins.check(item)
    states = []
    for case, recorded in zip(original['cases'], gate['facts'], strict=True):
        assert case['id'] == recorded['id']
        state = source.inspect(pins.check(recorded['source']), pins.check(recorded['getter']), case)
        assert state['protected_prefix_sha256'] == recorded['protected_prefix_sha256']
        assert normalized(state['maps']) == recorded['maps']
        assert state['roots'] == recorded['logical_ROW_roots']
        assert normalized(state['relationships']) == recorded['relationships']
        assert state['catalog_long_values'] == recorded['catalog_long_values']
        states.append(state)
    # Both complete originals qualify before deriving either image or interpreting AFTERs.
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['complete_images'] == 2
    assert expected['public_AFTER_inputs'] == expected['build_inputs'] == expected['native_candidate_inputs'] == 0
    requests = store.load(PUBLIC + '/requests.json')
    assert requests == {case['id']: case['requests'] for case in original['cases']}
    images = {}
    for state, case, item in zip(states, original['cases'], expected['cases'], strict=True):
        assert item['id'] == case['id']
        raw, details = model.derive(state, requests[case['id']])
        assert raw == pins.check(item['expected']).read_bytes()
        assert normalized(details) == store.load(pins.check(item['details']))
        images[case['id']] = raw
    reread_plan = store.load(EXPECTED_READ + '/plan.json')
    reread = report(store, pins, EXPECTED_READ_WORK + '/report.json', 2)
    assert len(reread['proofs']) == 2
    for state, case, expected_call, call, recorded in zip(states, original['cases'], reread_plan['cases'], reread['commands'], reread['proofs'], strict=True):
        assert call['argv'] == expected_call['argv']
        assert call['argv'] == original['java_prefix'] + ['inspect', expected_call['expected']['path'], case['selected']]
        assert pins.check(call['stderr']).read_bytes() == b''
        compare_recorded(store, pins, state, case, pins.check(expected_call['expected']),
                         pins.check(call['stdout']), recorded['facts'])
    receipt, git = build.verify(store, pins, args.verify_git)
    public_plan = store.load(REFERENCE + '/plan.json')
    assert public_plan['cases'] == original['cases']
    assert public_plan['commands'] == [command for case in original['cases'] for command in case['future_reference']]
    public = report(store, pins, REFERENCE_WORK + '/report.json', 4)
    assert len(public['proofs']) == 2
    for call, expected_call in zip(public['commands'], public_plan['commands'], strict=True):
        assert call['argv'] == expected_call['argv'] and call['label'] == expected_call['id']
        assert pins.check(call['stderr']).read_bytes() == b''
    difference_report = store.load(REFERENCE + '/public-differences-first.json')
    differences = []
    for position, (state, case, recorded, classified) in enumerate(zip(states, original['cases'], public['proofs'], difference_report['cases'], strict=True)):
        path = store.path(case['future_reference'][0]['argv'][-2])
        getter = pins.check(public['commands'][position * 2 + 1]['stdout'])
        compare_recorded(store, pins, state, case, path, getter, recorded['facts'])
        public_raw = path.read_bytes()
        expected_raw = images[case['id']]
        actual = [(offset, left, right) for offset, (left, right) in enumerate(zip(expected_raw, public_raw, strict=True)) if left != right]
        assert actual == [(item['offset'], item['expected'], item['public']) for item in classified['differences']]
        assert classified['difference_counts'] == {'new_NULL_fixed_payload': 2, 'vacated_FK_leaf_wire': 3}
        assert len(actual) == 5
        differences.append(len(actual))
    native_plan = store.load(NATIVE + '/plan.json')
    seeds = store.load(NATIVE + '/seed-freeze.json')
    native = report(store, pins, NATIVE_WORK + '/report.json', 2)
    assert native['phase'] == 'native' and len(native['whole_images']) == 2
    for state, case, planned, seed, call, item in zip(states, original['cases'], native_plan['cases'], seeds['outputs'], native['commands'], native['whole_images'], strict=True):
        row = state['tables']['Parents']['rows'][1]
        root = row['page'] << 8 | row['slot']
        ordinal = state['roots']['Parents'].index(root)
        assert seed['logical_ROW'] == root and seed['live_row_ordinal'] == ordinal
        tsv = pins.check(seed['request']).read_text()
        assert list(csv.reader(tsv.splitlines(), delimiter='\t')) == [[str(ordinal), row['raw'].hex()]]
        assert call['argv'] == planned['native']['argv'] == [
            receipt['artifact']['path'], case['source'], planned['output'], '--cascade',
            '--group', 'Parents', seed['request']['path']]
        assert pins.check(call['stdout']).read_bytes() == f"candidate_deleted_rows={planned['deleted_rows']}\n".encode()
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert pins.check(item['image']).read_bytes() == images[case['id']]
    # Both complete images are required before any separately bound fresh reader.
    binding = store.load(NATIVE + '/getter-binding.json')
    assert binding['native_report'] == store.pin(NATIVE_WORK + '/report.json')
    fresh = report(store, pins, GETTERS + '/report.json', 2)
    assert fresh['phase'] == 'getters' and len(fresh['proofs']) == 2
    final_rows = 0
    final_pairs = 0
    for state, case, planned, call, recorded in zip(states, original['cases'], native_plan['cases'], fresh['commands'], fresh['proofs'], strict=True):
        assert call['argv'] == planned['getter']['argv'] == original['java_prefix'] + ['inspect', planned['output'], case['selected']]
        assert pins.check(call['stderr']).read_bytes() == b''
        path = store.path(planned['output'])
        assert path.read_bytes() == images[case['id']]
        compare_recorded(store, pins, state, case, path, pins.check(call['stdout']), recorded['facts'])
        tables = source.parse(pins.check(call['stdout']).read_text())[0]
        for name in case['before']:
            table = tables[name]
            final_rows += table['count']
            physical = {}
            for index in table['indexes'].values():
                physical[index['physical']] = index['entries']
            final_pairs += sum(physical.values())
    pins.terminal()
    return dict(verified=True, available=True, source_capture_commands=5, saved_source_gates=2,
                source_request_complete_images_before_AFTER=2, expected_inert_reads=2,
                public_reference_commands=4, public_transition_proofs=2,
                public_model_differing_bytes=differences, native_calls=2, fresh_inert_inspections=2,
                final_user_rows=final_rows, final_physical_key_ROW_pairs=final_pairs,
                deleted_logical_roots=3, surviving_NULL_assignments=4,
                pure_failed_checkpoints_preserved=2, actual_native_JVM_Cargo_calls_by_verifier=0,
                pin_occurrences=pins.occurrences, distinct_paths=len(pins.items),
                build_artifact=receipt['artifact'], Git=git)


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
