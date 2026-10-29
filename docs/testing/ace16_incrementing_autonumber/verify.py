"""Saved AutoNumber evidence replay; default starts no subprocess or cached code."""
import argparse
import ast
import csv
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_external_long_values.evidence import Pins, report
from ace16_incrementing_autonumber import build, model, reference, source

ROOT = Path(__file__).resolve().parents[3]
PUBLIC = 'docs/testing/ace16_incrementing_autonumber'
SOURCE = '.cache/ace16-incrementing-autonumber-insert-next-20261028'
SOURCE_WORK = '.cache/ace16-incrementing-autonumber-insert-source-first-20261028'
FIRST_EXPECTED = '.cache/ace16-incrementing-autonumber-insert-expected-first-20261028'
EXPECTED = '.cache/ace16-incrementing-autonumber-insert-expected-bytes-first-20261028'
REFERENCE = '.cache/ace16-incrementing-autonumber-insert-reference-first-20261028'
NATIVE = '.cache/ace16-incrementing-autonumber-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-incrementing-autonumber-candidates-first-20261028'
FRESH = '.cache/ace16-incrementing-autonumber-fresh-getters-next-20261028'
FRESH_WORK = '.cache/ace16-incrementing-autonumber-getters-corrected-first-20261028'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def normalized(value):
    return json.loads(json.dumps(value))


def maintained(store, pins):
    for private, public in [('source.py', 'source.py'), ('model_bytes.py', 'model.py'),
                            ('reference.py', 'reference.py')]:
        original = pins.check(store.pin(SOURCE + '/' + private)).read_text()
        expected = original
        if private in ('source.py', 'model_bytes.py'):
            expected = expected.replace('parents[2]', 'parents[3]')
        if private == 'reference.py':
            expected = expected.replace('import source\n', 'from ace16_incrementing_autonumber import source\n')
        actual = pins.check(store.pin(PUBLIC + '/' + public)).read_text()
        assert actual == expected, public
        definitions = lambda text: [ast.dump(node, include_attributes=False)
                                   for node in ast.parse(text).body
                                   if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        assert definitions(actual) == definitions(original)
    for name in ['source-plan.json', 'IncrementingAutoNumberFixture.java',
                 'requests/fresh.tsv', 'requests/highest.tsv', 'requests/all.tsv', 'requests/pressure.tsv']:
        assert pins.check(store.pin(PUBLIC + '/' + name)).read_bytes() == pins.check(store.pin(SOURCE + '/' + name)).read_bytes()
    # The first reader protocol never ran: its source remained the public stream
    # validator. The corrected executed reader requires the actual CLI response.
    public_text = pins.check(store.pin(SOURCE + '/reference.py')).read_text()
    assert pins.check(store.pin(NATIVE + '/proof.py')).read_text() == public_text
    start = public_text.index('    # Public reference callers supply')
    end = public_text.index('    tables, metadata, properties', start)
    replacement = '''    assert append_stdout is not None
    output = Path(append_stdout).read_text().splitlines()
    expected_output = [f'candidate_changed_rows={len(generated)}', 'generated_column_number=1']
    expected_output += [f'generated_value[{position}]={identity}'
                        for position, identity in enumerate(generated)]
    assert output == expected_output
'''
    assert pins.check(store.pin(FRESH + '/proof.py')).read_text() == public_text[:start] + replacement + public_text[end:]


def preparation_history(store, pins):
    initial = store.load(FIRST_EXPECTED + '/report.json')
    assert initial['completed'] is False and initial['child_commands'] == 0
    assert len(initial['expected']) == 3 and initial['pin_errors'] == []
    assert initial['active_case'] == 'V2016-pressure'
    assert initial['failure'] == "TypeError: cannot use 'bytearray' as a set element (unhashable type: 'bytearray')"
    for item in initial['inputs']:
        pins.check(item)
    original = pins.check(store.pin(SOURCE + '/model.py')).read_text()
    corrected = pins.check(store.pin(SOURCE + '/model_bytes.py')).read_text()
    assert original.replace("wire.index_wire.tree(raw, table['indexes'][name]['root'], owner)",
                            "wire.index_wire.tree(bytes(raw), table['indexes'][name]['root'], owner)") == corrected
    complete = store.load(EXPECTED + '/report.json')
    for old in initial['expected']:
        new = next(item for item in complete['expected'] if item['id'] == old['id'])
        assert pins.check(old['image']).read_bytes() == pins.check(new['image']).read_bytes()
    correction = store.load(FRESH + '/correction.json')
    assert correction['observed_before_fresh_reader_calls'] and correction['native_calls_repeated'] == 0
    assert correction['standalone_reader_failure_log'] is False
    pins.check(correction['original_freeze'])
    authoring = store.load(SOURCE + '/reference-freeze-authoring-failure.json')
    assert 'zero child calls' in authoring['followup']
    return dict(first_pure_attempt_images=3, pure_reader_bytes_boundary_corrected=True,
                first_three_images_unchanged=True, source_pins_changed=False,
                pre_freeze_reference_guard_error=True, pre_reader_stream_contract_correction=True,
                original_reader_calls=0, repeated_native_calls=0)


def request_rows(store, pins, case):
    path = pins.check(store.pin(PUBLIC + '/requests/' + case['state'] + '.tsv'))
    result = []
    with path.open() as handle:
        for fields in csv.reader(handle, delimiter='\t'):
            assert len(fields) == 4
            values = []
            for kind, text in zip(fields[::2], fields[1::2], strict=True):
                if kind == 'null':
                    assert text == ''
                    values.append(None)
                else:
                    assert kind == 'int32'
                    value = int(text)
                    assert -(1 << 31) <= value < 1 << 31
                    values.append(value)
            result.append(values)
    assert result == case['requests']
    return result


def public_difference(expected, public, observed, owner):
    assert len(expected) == len(public)
    offsets = [position for position, (left, right) in enumerate(zip(expected, public)) if left != right]
    counts = {page: sum(position // 4096 == page for position in offsets)
              for page in sorted({position // 4096 for position in offsets})}
    populations = []
    for item in observed['indexes']:
        public_nodes, public_leaves, _ = source.native.index_wire.tree(public, item['root'], owner)
        model_nodes, model_leaves, _ = source.native.index_wire.tree(expected, item['root'], owner)
        populations.append(dict(physical=item['physical'],
                                public=[len(public_nodes[page]['entries']) for page in public_leaves],
                                engineering=[len(model_nodes[page]['entries']) for page in model_leaves]))
    return dict(differing_bytes=len(offsets), by_page=counts, leaf_populations=populations,
                absent_score_payloads=[dict(identity=item['identity'], hex=item['absent_score_payload'])
                                       for item in observed['new_rows'] if item['absent_score_payload'] is not None])


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    anchors = store.load(PUBLIC + '/anchors.json')['phase_anchors']
    # SOURCE/expected pins only until ALL eight independent reconstructions are
    # complete. Public AFTER and candidate bytes are read in later phases.
    early = [path for path in anchors if path.startswith(SOURCE + '/') or path.startswith(SOURCE_WORK + '/')
             or path.startswith(FIRST_EXPECTED + '/') or path.startswith(EXPECTED + '/')]
    for path in early:
        pins.check(anchors[path])
    for name in ['source-freeze.json', 'model-freeze.json', 'model-bytes-freeze.json', 'expected-freeze.json']:
        pins.closure(SOURCE + '/' + name)
    maintained(store, pins)
    history = preparation_history(store, pins)
    plan = store.load(PUBLIC + '/source-plan.json')
    captured = report(store, pins, SOURCE_WORK + '/report.json', 17)
    assert captured['source_qualification'] is False and len(captured['images']) == 8
    for actual, expected in zip(captured['commands'], plan['commands'], strict=True):
        assert actual['label'] == expected['id'] and actual['argv'] == expected['argv']
        assert pins.check(actual['stderr']).read_bytes() == b''
    gates = store.load(SOURCE + '/source-gates-first.json')
    assert gates['source_qualification'] and gates['source_gates'] == 8
    assert gates['actual_child_commands'] == gates['constructor_images'] == 0
    assert gates['stored_mode_detection'] is False
    for item in gates['pins']:
        pins.check(item)
    expected_report = store.load(EXPECTED + '/report.json')
    assert expected_report['completed'] and expected_report['pin_errors'] == []
    assert expected_report['child_commands'] == 0 and len(expected_report['expected']) == 8
    states, reconstructed = {}, {}
    for case, saved in zip(plan['cases'], expected_report['expected'], strict=True):
        assert case['id'] == saved['id']
        fact = next(item for item in gates['facts'] if item['id'] == case['id'])
        request_rows(store, pins, case)
        state = source.inspect(pins.check(fact['source']), pins.check(fact['getter']), case)
        assert state['counter_last'] == fact['counter_last']
        assert normalized(state['logical']) == fact['logical']
        image, facts = model.derive(state, case)
        assert normalized(facts) == saved['facts']
        assert image == pins.check(saved['image']).read_bytes()
        states[case['id']] = state
        reconstructed[case['id']] = image
    assert sum(map(len, reconstructed.values())) == 3571712
    # Only now validate/read public AFTERs, artifacts and native results.
    for path, item in anchors.items():
        if path not in early:
            pins.check(item)
    for path in [SOURCE + '/reference-freeze.json', NATIVE + '/freeze.json', FRESH + '/freeze.json']:
        pins.closure(path)
    receipt, git = build.verify(store, pins, args.verify_git)
    assert receipt['expected_images_frozen_before_build'] == store.pin(SOURCE + '/model-bytes-freeze.json')
    assert receipt['expected_report_frozen_before_build'] == store.pin(EXPECTED + '/report.json')
    public = report(store, pins, REFERENCE + '/report.json', 16)
    reference_plan = store.load(SOURCE + '/reference-plan.json')
    assert len(public['proofs']) == len(public['images']) == 8
    assert public['source_gates_before_first_append'] == 8
    for actual, planned in zip(public['commands'], reference_plan['commands'], strict=True):
        assert actual['argv'] == planned['argv'] and actual['label'] == planned['id']
        assert pins.check(actual['stderr']).read_bytes() == b''
    differences = []
    for case, proof in zip(plan['cases'], public['proofs'], strict=True):
        assert case['id'] == proof['id']
        after = REFERENCE + '/' + case['id'] + '-reference.accdb'
        index = plan['cases'].index(case) * 2
        proof_case = dict(case, before_getter=str(store.path(SOURCE_WORK + '/' + case['id'] + '-before-inspect.stdout')))
        observed = reference.compare(states[case['id']], proof_case, pins.check(store.pin(after)),
                                     pins.check(public['commands'][index + 1]['stdout']),
                                     pins.check(public['commands'][index]['stdout']))
        assert normalized(observed) == proof['facts']
        difference = public_difference(reconstructed[case['id']], store.path(after).read_bytes(), observed,
                                       states[case['id']]['tables']['Items']['definition'])
        assert difference['differing_bytes'] == (3152 if case['state'] == 'pressure' else 0)
        differences.append(dict(id=case['id'], **difference))
    native = report(store, pins, args.producer_report, 8)
    getters = report(store, pins, args.getter_report, 8)
    runtime = store.load(NATIVE + '/plan.json')
    fresh_runtime = store.load(FRESH + '/plan.json')
    assert runtime['artifact'] == fresh_runtime['artifact'] == receipt['artifact']
    assert runtime['classes'] == fresh_runtime['classes'] == captured['classes']
    assert len(native['whole_images']) == len(getters['proofs']) == 8
    binding = store.load(FRESH + '/getter-binding.json')
    assert binding['native_report'] == store.pin(args.producer_report)
    for item in binding['inputs']:
        pins.check(item)

    def all_candidates():
        for case in runtime['cases']:
            expected = reconstructed[case['id']]
            assert pins.check(case['expected']).read_bytes() == expected
            assert pins.check(store.pin(case['output'])).read_bytes() == expected

    all_candidates()
    for case, declared, native_command, getter_command, image, proof in zip(
            plan['cases'], runtime['cases'], native['commands'], getters['commands'],
            native['whole_images'], getters['proofs'], strict=True):
        assert case['id'] == declared['id'] == image['id'] == proof['id']
        assert declared['origin'] == case
        assert native_command['argv'] == declared['native']['argv']
        assert getter_command['argv'] == declared['getter']['argv']
        generated = [case['expected_last'] + position + 1 for position in range(len(case['requests']))]
        wanted = [f'candidate_changed_rows={len(generated)}', 'generated_column_number=1']
        wanted += [f'generated_value[{position}]={identity}' for position, identity in enumerate(generated)]
        assert pins.check(native_command['stdout']).read_text().splitlines() == wanted
        assert pins.check(native_command['stderr']).read_bytes() == NOTICE
        assert pins.check(getter_command['stderr']).read_bytes() == b''
        candidate = pins.check(image['image'])
        assert candidate == store.path(declared['output'])
        all_candidates()
        proof_case = dict(case, before_getter=str(store.path(SOURCE_WORK + '/' + case['id'] + '-before-inspect.stdout')))
        actual = reference.compare(states[case['id']], proof_case, candidate,
                                   pins.check(getter_command['stdout']))
        assert normalized(actual) == proof['facts']
        all_candidates()
    pins.terminal()
    return dict(verified=True, available=True, source_capture_commands=17, saved_source_gates=8,
                source_request_images_before_AFTER=8, public_reference_commands=16,
                public_reference_proofs=8, native_calls=8, fresh_combined_getter_map_calls=8,
                generated_rows=20, generated_column_number=1, complete_candidate_bytes=3571712,
                public_differences=differences, preparation_history=history,
                build_artifact=receipt['artifact'], Git=git,
                stored_mode_detection=False, native_Access_qualification=False,
                pin_occurrences=pins.occurrences, distinct_paths=len(pins.items),
                actual_native_JVM_Cargo_calls_by_verifier=0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default=NATIVE_WORK + '/report.json')
    parser.add_argument('--getter-report', default=FRESH_WORK + '/report.json')
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
