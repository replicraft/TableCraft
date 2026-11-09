"""Replay retained IgnoreNULL UPDATE evidence; default starts no child process."""
import argparse
import ast
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_external_long_values.evidence import Pins, report
from ace16_fixed_ignore_null_updates import build, model, native_reference, reference, source, source_warnings

ROOT = Path(__file__).resolve().parents[3]
PUBLIC = 'docs/testing/ace16_fixed_ignore_null_updates'
SOURCE = '.cache/ace16-fixed-ignore-null-update-source-next-20261029'
SOURCE_WORK = '.cache/ace16-fixed-ignore-null-update-source-first-20261029'
CONSTRUCTOR = '.cache/ace16-fixed-ignore-null-update-expected-next-20261029'
EXPECTED = '.cache/ace16-fixed-ignore-null-update-expected-first-20261029'
REFERENCE = '.cache/ace16-fixed-ignore-null-update-reference-first-20261029'
CONTINUED = '.cache/ace16-fixed-ignore-null-update-reference-continuation-first-20261029'
NATIVE = '.cache/ace16-fixed-ignore-null-update-acceptance-next-20261029'
NATIVE_WORK = '.cache/ace16-fixed-ignore-null-update-native-first-20261029'
FRESH_WORK = '.cache/ace16-fixed-ignore-null-update-getters-first-20261029'
NOTICES = {
    'preview_ace16_fixed': b'Engineering candidate: data-macro semantics and native Access qualification pending.\n',
    'preview_ace16_batch': b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\n',
}
ERRORS = {
    'IndexedColumn': b'Error: Unsupported(IndexedColumn)\n',
    'DuplicateKey': b'Error: Inspect(IndexConstraint(DuplicateKey))\n',
    'NullKey': b'Error: Inspect(IndexConstraint(NullKey))\n',
}


def normalized(value):
    return json.loads(json.dumps(value))


def identity(store, pins):
    conversions = [
        (SOURCE + '/source.py', 'source.py'),
        (CONSTRUCTOR + '/model.py', 'model.py'),
        (SOURCE + '/reference_counter.py', 'reference.py'),
        (NATIVE + '/proof.py', 'native_reference.py'),
    ]
    for original_path, public in conversions:
        original = pins.check(store.pin(original_path)).read_text()
        expected = original
        if public == 'source.py':
            expected = expected.replace('parents[2]', 'parents[3]')
        elif public == 'model.py':
            start = expected.index('import importlib.util')
            end = expected.index('\nPAGE =', start)
            expected = expected[:start] + ('from pathlib import Path\n\n'
                'from ace16_fixed_ignore_null_updates import source\n') + expected[end:]
        else:
            expected = expected.replace('import source\n', 'from ace16_fixed_ignore_null_updates import source\n')
        actual = pins.check(store.pin(PUBLIC + '/' + public)).read_text()
        assert actual == expected, public
        definitions = lambda text: [ast.dump(node, include_attributes=False)
                                   for node in ast.parse(text).body
                                   if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        assert definitions(actual) == definitions(original)
    for name in ['plan.json', 'source-plan-warning.json', 'source_warnings.py', 'IgnoredNullUpdateFixture.java']:
        assert pins.check(store.pin(PUBLIC + '/' + name)).read_bytes() == pins.check(store.pin(SOURCE + '/' + name)).read_bytes()


def public_difference(original, case, expected, observed):
    assert len(expected) == len(observed)
    table = original['tables'][case['action']['table']]
    fields = {field['name']: field for field in original['declarations'][case['action']['table']]['columns']}
    absent_payloads = set()
    for identity, changes in case['action']['updates']:
        row = table['rows'][identity]
        begin, end = model.record_range(original['raw'], row, table['definition'])
        for name, value in changes.items():
            if value is not None:
                continue
            field = fields[name]
            assert expected[end - 1] & (1 << field['number']) == 0
            assert observed[end - 1] & (1 << field['number']) == 0
            offset = begin + 2 + source.native.number(bytes.fromhex(field['raw_hex']), 21)
            absent_payloads.update(range(offset, offset + 4))
    offsets = [position for position, (left, right) in enumerate(zip(expected, observed, strict=True))
               if left != right]
    assert all(position in absent_payloads for position in offsets)
    return dict(id=case['id'], differing_bytes=len(offsets),
                clear_mask_payload_bytes=len(offsets), other_differences=0)


def native_report(store, pins, path):
    value = store.load(path)
    assert value['completed'] and value['phase'] == 'native' and value['pin_errors'] == []
    assert len(value['commands']) == 34 and len(value['whole_images']) == 16 and len(value['refusals']) == 18
    for item in value['inputs'] + value['artifacts']:
        pins.check(item)
    for command in value['commands']:
        assert command['status'] == command['expected_status']
        assert command['status'] in (0, 1) and command['stopped_for'] is None
        assert command['capture_errors'] == [] and command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16777216
        assert command['observed_stream_bytes'] == [command['stdout']['bytes'], command['stderr']['bytes']]
        assert max(command['observed_stream_bytes']) <= 16777216
        pins.check(command['stdout'])
        pins.check(command['stderr'])
    return value


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    anchors = store.load(PUBLIC + '/anchors.json')
    for item in anchors['early'].values():
        pins.check(item)
    for path in [SOURCE + '/source-freeze.json', SOURCE + '/source-warning-freeze.json',
                 CONSTRUCTOR + '/freeze.json', EXPECTED + '/freeze.json']:
        pins.closure(path)
    identity(store, pins)
    plan = store.load(PUBLIC + '/plan.json')
    source_plan = store.load(PUBLIC + '/source-plan-warning.json')
    captured = report(store, pins, SOURCE_WORK + '/report.json', 5)
    assert captured['source_qualification'] is False and captured['public_reference_commands'] == 0
    for actual, expected in zip(captured['commands'], source_plan['commands'], strict=True):
        assert actual['argv'] == expected['argv'] and actual['label'] == expected['id']
        stderr = pins.check(actual['stderr']).read_bytes()
        if actual['label'].endswith('-create'):
            case = next(case for case in source_plan['cases'] if actual['label'] == case['profile'] + '-create')
            source_warnings.require_create(stderr, case['source'])
        else:
            assert stderr == b''
    gates = store.load(SOURCE + '/source-gates-first.json')
    assert gates['source_qualification'] and gates['source_gates'] == 12 and gates['sentinel_gates'] == 2
    assert gates['actual_child_commands'] == gates['constructor_images'] == 0
    for item in gates['pins']:
        pins.check(item)
    classified = store.load(CONSTRUCTOR + '/classification.json')
    assert classified['positives'] == 16 and classified['refusals'] == 18
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['pin_errors'] == [] and expected['child_commands'] == 0
    assert len(expected['expected']) == 16 and len(expected['refusals']) == 18
    assert expected['source_gates_before_first_constructor'] == 12
    assert expected['sentinel_gates_before_first_constructor'] == 2
    states = {}
    for saved in gates['facts']:
        state = model.before(pins.check(saved['source']), pins.check(saved['getter']), saved['profile'])
        assert normalized(state['facts']) == saved['facts']
        states[saved['profile']] = state
    reconstructed = {}
    classifications = {}
    for case in classified['cases']:
        intent = model.intent(states[case['profile']], case)
        classifications[case['id']] = case
        assert intent['outcome'] == case['outcome']
        if case['outcome'] != 'supported':
            continue
        image, facts = model.derive(states[case['profile']], case)
        saved = next(item for item in expected['expected'] if item['id'] == case['id'])
        assert saved['action'] == case['action'] and normalized(facts) == store.load(pins.check(saved['details']))
        assert image == pins.check(saved['image']).read_bytes()
        reconstructed[case['id']] = image
    assert len(reconstructed) == 16
    # No AFTER image was an input to any of the sixteen constructions above.
    for item in anchors['late'].values():
        pins.check(item)
    for path in [SOURCE + '/reference-freeze.json', SOURCE + '/reference-continuation-freeze.json', NATIVE + '/freeze.json']:
        pins.closure(path)
    receipt, git = build.verify(store, pins, args.verify_git)
    assert receipt['expected_images_frozen_before_build'] == store.pin(CONSTRUCTOR + '/freeze.json')
    assert receipt['expected_report_frozen_before_build'] == store.pin(EXPECTED + '/report.json')
    failed = report(store, pins, REFERENCE + '/report.json', 2, completed=False)
    assert failed['failure'] == 'AssertionError: declarations' and failed['proofs'] == []
    saved_first = store.load(SOURCE + '/saved-first-counter-proof.json')
    assert saved_first['replayed'] and saved_first['actual_child_commands'] == 0
    assert saved_first['completed_original_children'] == 2 and saved_first['original_report'] == store.pin(REFERENCE + '/report.json')
    continued = report(store, pins, CONTINUED + '/report.json', 66)
    original_plan = store.load(SOURCE + '/reference-plan.json')
    continuation_plan = store.load(SOURCE + '/reference-continuation-plan.json')
    commands = failed['commands'] + continued['commands']
    assert continuation_plan['commands'] == original_plan['commands'][2:]
    assert len(commands) == 68 and len(continued['proofs']) == 33
    for actual, declared in zip(commands, original_plan['commands'], strict=True):
        assert actual['argv'] == declared['argv'] and actual['label'] == declared['id']
    observations = []
    public_differences = []
    for position, case in enumerate(original_plan['cases']):
        action, getter = commands[2 * position:2 * position + 2]
        assert pins.check(getter['stderr']).read_bytes() == b''
        facts = reference.compare(
            store.path(case['source']), store.path(case['before_getter']), store.path(case['output']),
            pins.check(getter['stdout']), case['profile'], case['action'],
            pins.check(action['stdout']), pins.check(action['stderr']),
        )
        saved = saved_first['facts'] if position == 0 else continued['proofs'][position - 1]['facts']
        assert normalized(facts) == saved
        observations.append(dict(id=case['id'], **facts))
        if case['id'] in reconstructed:
            public_differences.append(public_difference(
                states[case['profile']], case, reconstructed[case['id']], store.path(case['output']).read_bytes(),
            ))
    native = native_report(store, pins, args.producer_report)
    getters = report(store, pins, args.getter_report, 16)
    runtime = store.load(NATIVE + '/plan.json')
    binding = store.load(NATIVE + '/getter-binding.json')
    assert binding['native_report'] == store.pin(args.producer_report)
    for item in binding['inputs']:
        pins.check(item)
    assert runtime['artifacts'] == receipt['artifacts'] and runtime['classes'] == captured['classes']
    assert len(getters['proofs']) == 16

    def all_images():
        for case in runtime['cases']:
            if case['expected_status'] == 0:
                assert pins.check(store.pin(case['output'])).read_bytes() == reconstructed[case['id']]
            else:
                assert not store.path(case['output']).exists()

    all_images()
    positives = []
    for case, command in zip(runtime['cases'], native['commands'], strict=True):
        original = classifications[case['id']]
        assert case['action'] == original['action'] and case['profile'] == original['profile']
        assert command['argv'] == case['native']['argv']
        stdout = pins.check(command['stdout']).read_bytes()
        stderr = pins.check(command['stderr']).read_bytes()
        if case['expected_status'] == 0:
            assert original['outcome'] == 'supported'
            assert stdout == f"candidate_changed_rows={case['changed_rows']}\n".encode()
            assert stderr == NOTICES[case['consumer']]
            positives.append((case, command))
        else:
            assert case['expected_status'] == 1 and original['outcome'] in ERRORS
            assert stdout == b'' and stderr == ERRORS[original['outcome']]
            assert not store.path(case['output']).exists()
    assert len(positives) == 16
    for (case, command), getter, saved in zip(positives, getters['commands'], getters['proofs'], strict=True):
        assert getter['argv'] == case['getter']['argv'] and pins.check(getter['stderr']).read_bytes() == b''
        all_images()
        facts = native_reference.compare(
            pins.check(case['source']), pins.check(case['before_getter']), store.path(case['output']),
            pins.check(getter['stdout']), case['profile'], case['action'],
            pins.check(command['stdout']), pins.check(command['stderr']),
        )
        assert saved == dict(id=case['id'], facts=normalized(facts))
        all_images()
    pins.terminal()
    return dict(verified=True, available=True, source_commands=5, source_targets=12,
                sentinel_gates=2, source_request_images_before_AFTER=16,
                public_reference_commands=68, public_reference_proofs=34,
                initial_public_commands_preserved=2, continuation_commands=66,
                native_commands=34, native_exact_images=16, native_typed_refusals=18,
                fresh_readers=16, complete_candidate_bytes=sum(map(len, reconstructed.values())),
                public_observations=observations, public_differences=public_differences, Git=git,
                pin_occurrences=pins.occurrences, distinct_paths=len(pins.items),
                actual_native_JVM_Cargo_calls_by_verifier=0, native_Access_qualification=False)


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
        print(json.dumps(dict(verified=False, available=False, missing=str(error.filename)), indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps(dict(verified=False, available=True, reason=type(error).__name__ + ': ' + str(error)), indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
