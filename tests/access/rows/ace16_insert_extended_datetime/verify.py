"""Saved Extended Date/Time proof; no producer, JVM or default subprocess."""
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
from ace16_insert_extended_datetime import build, model, reference, source

ROOT = Path(__file__).resolve().parents[3]
SOURCE = '.cache/ace16-extended-datetime-insert-source-next-20261028'
SOURCE_WORK = '.cache/ace16-extended-datetime-insert-reference-first-20261028'
EXPECTED = '.cache/ace16-extended-datetime-insert-expected-first-20261028'
REFERENCE = '.cache/ace16-extended-datetime-insert-public-next-20261028'
REFERENCE_WORK = '.cache/ace16-extended-datetime-insert-public-first-20261028'
BUILD = '.cache/ace16-insert-extended-datetime-isolated-build-20261028'
PREPARE_FAILURE = '.cache/ace16-insert-extended-datetime-build-20261028/prepare-failure.json'
NATIVE = '.cache/ace16-extended-datetime-insert-acceptance-next-20261028'
NATIVE_WORK = '.cache/ace16-extended-datetime-insert-candidate-first-20261028'
GETTERS = '.cache/ace16-extended-datetime-insert-getters-first-20261028'
PUBLIC = 'docs/testing/ace16_insert_extended_datetime'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    '.cache/ace16-extended-datetime-insert-source-next-20261028/plan-helper-freeze.json':
        '9cabdc471760cc75011458941e7a0a27aab472c3bf4235f42c760dc753d8d9e2',
    '.cache/ace16-extended-datetime-insert-source-next-20261028/source-freeze-v2.json':
        'cc5ea92aa2e914797aed59222210b2d22a6bb8cede1a7efe19fe78bb79772737',
    '.cache/ace16-extended-datetime-insert-source-next-20261028/source-gates-first.json':
        'a0a4f325cce68490dfba85fc47f88935b318f4d31e41903e2cef9861455910bc',
    '.cache/ace16-extended-datetime-insert-source-next-20261028/model-freeze.json':
        '55c59af233512b9d690791826a48b1a926b7833136b7975dac23108e283fe967',
    '.cache/ace16-extended-datetime-insert-reference-first-20261028/report.json':
        '381436b9c7a7a7526b99b217d0597a95e3bc5b1c4e4b97295839c324d74a50d6',
    '.cache/ace16-extended-datetime-insert-expected-first-20261028/report.json':
        'f2c53d7fe90a5ad6c62eaa8bce219d34671b5f893fdc588fe6dbc672c5bf03f0',
    '.cache/ace16-extended-datetime-insert-public-next-20261028/freeze.json':
        '0dbb2aceae1c693b5a70ab3f51628e2ce7249cf4f85e574d2a854b02653ee354',
    '.cache/ace16-extended-datetime-insert-public-first-20261028/report.json':
        'd769726b798c05048fd0c36dee5afe53009a71ebf76d9d55a015a38fffc69432',
    '.cache/ace16-insert-extended-datetime-isolated-build-20261028/freeze.json':
        'ad7d0e56363cf633d20b405a516b6e2e308075b05c56d7d616f262c811f959ba',
    'docs/testing/results/ace16-insert-extended-datetime-example-build-2026-10-28.json':
        '7e9fb86575da3b07fc6c3718af088e22832510601b59d2d7ad3a3ecaf70affe4',
    '.cache/ace16-insert-extended-datetime-build-20261028/prepare-failure.json':
        '85d3c75c3959cd04d5fe1ba215a3bc13cdcadbee7c9654e6ddd367547e756a3d',
    '.cache/ace16-extended-datetime-insert-acceptance-next-20261028/freeze.json':
        'd5512610f878267a83c9411d2d40239f37316c749eb4ba1f6e6693cacb715ed8',
    '.cache/ace16-extended-datetime-insert-acceptance-next-20261028/getter-binding.json':
        'e69ee576ef6b716cea9b80a6b8ab8805ededbf54be366b3594286ba135574ce7',
    '.cache/ace16-extended-datetime-insert-candidate-first-20261028/report.json':
        '660ef48630ad1d0406d2cd8be213c5b25b9006e3ecd2db07d86393619d4974ee',
    '.cache/ace16-extended-datetime-insert-getters-first-20261028/report.json':
        '35e3b8626ecd68f5942fea2fd4dec2e780f0245473b1f7cdafa016cdf0499b57',
}


def maintained(store, pins):
    for owner, name in [(SOURCE, 'source.py'), (SOURCE, 'model.py'), (REFERENCE, 'reference.py'),
                         (SOURCE, 'plan.json'), (SOURCE, 'requests.tsv'),
                         (SOURCE, 'ExtendedDateTimeInsertFixture.java')]:
        private = 'proof.py' if name == 'reference.py' else name
        original = pins.check(store.pin(owner + '/' + private)).read_text()
        expected = original
        if name == 'source.py':
            expected = expected.replace('parents[2]', 'parents[3]')
        if name in ('model.py', 'reference.py'):
            expected = expected.replace('import source\n',
                'from ace16_insert_extended_datetime import source\n')
        actual = pins.check(store.pin(PUBLIC + '/' + name)).read_text()
        assert actual == expected, name
        if name.endswith('.py'):
            def definitions(text):
                return [ast.dump(node, include_attributes=False) for node in ast.parse(text).body
                        if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
            assert definitions(actual) == definitions(original)
    bounded = pins.check(store.pin('docs/testing/parent_index_split_originals/BoundedFileChannel.java'))
    assert bounded.read_bytes() == pins.check(store.pin(SOURCE + '/BoundedFileChannel.java')).read_bytes()


def preparation_history(store, pins):
    preservation = store.load(SOURCE + '/before-qualifier-invariance/preservation.json')
    for item in preservation['preserved']:
        pins.check(dict(path=item['preserved_path'], bytes=item['bytes'], sha256=item['sha256']))
    prior = pins.check(store.pin(SOURCE + '/before-model-bootstrap/model.py')).read_text()
    current = pins.check(store.pin(SOURCE + '/model.py')).read_text()
    assert prior.replace('from ace16_insert_external_long_values import model as ordinary\nimport source',
                         'import source\nfrom ace16_insert_external_long_values import model as ordinary') == current
    assert pins.check(store.pin(SOURCE + '/before-model-bootstrap/prepare.py')).read_bytes() == pins.check(
        store.pin(SOURCE + '/prepare.py')).read_bytes()
    bootstrap = store.load(SOURCE + '/before-model-bootstrap/history.json')
    assert bootstrap['attempts'] == 2 and bootstrap['expected_images'] == bootstrap['fixture_native_Cargo_calls'] == 0
    failed = store.load(PREPARE_FAILURE)
    assert failed['completed'] is False and failed['phase'] == 'prepare-before-Cargo'
    assert failed['Cargo_commands'] == 0
    assert not failed['source_directory_created'] and not failed['target_directory_created']
    assert pins.check(failed['partial_archive']).read_bytes() == b''
    pins.check(failed['driver'])
    return dict(pre_execution_qualifier_revision=True, unfrozen_import_failures=2,
                import_failures_constructed_images=0, ENOSPC_before_Cargo=True,
                failed_attempt_Cargo_calls=0, repeated_completed_runtime_calls=0)


def request_rows(store, pins, origin):
    path = pins.check(store.pin(SOURCE + '/requests.tsv'))
    rows = []
    with path.open() as handle:
        for fields in csv.reader(handle, delimiter='\t'):
            assert len(fields) == 6
            row = []
            for kind, value in zip(fields[::2], fields[1::2], strict=True):
                if kind == 'null':
                    assert value == ''
                    row.append(None)
                elif kind == 'int32':
                    parsed = int(value)
                    assert -(2 ** 31) <= parsed < 2 ** 31
                    row.append(parsed)
                else:
                    assert kind == 'extended-date' and len(value) == 27
                    assert source.timestamp(model.encoded_timestamp(value)) == value
                    row.append(value)
            rows.append(row)
    assert rows == origin['requests'] and len(rows) == 6
    return rows


def replay(args):
    store = ArtifactStore(args.evidence_root)
    pins = Pins(store)
    store.path(args.producer_report).stat()
    store.path(args.getter_report).stat()
    for path, digest in FIXED.items():
        pins.fixed(path, digest)
    for path in [SOURCE + '/source-freeze-v2.json', SOURCE + '/model-freeze.json',
                 REFERENCE + '/freeze.json', BUILD + '/freeze.json',
                 NATIVE + '/freeze.json', NATIVE + '/getter-binding.json']:
        pins.closure(path)
    maintained(store, pins)
    history = preparation_history(store, pins)
    plan = store.load(SOURCE + '/plan.json')
    origin = plan['origins'][0]
    captured = report(store, pins, SOURCE_WORK + '/report.json', 3)
    assert captured['source_qualification'] is False and len(captured['images']) == 1
    for actual, declared in zip(captured['commands'], plan['source_commands'], strict=True):
        assert actual['argv'] == declared['argv'] and actual['label'] == declared['label']
        assert pins.check(actual['stderr']).read_bytes() == b''
    gate = store.load(SOURCE + '/source-gates-first.json')
    assert gate['source_qualification'] and gate['source_gates'] == 1
    assert gate['actual_child_commands'] == gate['expected_images_computed'] == 0
    assert gate['report'] == store.pin(SOURCE_WORK + '/report.json')
    state = source.inspect(pins.check(gate['source']), pins.check(gate['getter']))
    assert state['table']['definition'] == gate['facts']['definition']
    assert json.loads(json.dumps(state['census'])) == gate['facts']['census']
    rows = request_rows(store, pins, origin)
    expected = store.load(EXPECTED + '/report.json')
    assert expected['completed'] and expected['pin_errors'] == []
    assert expected['source_gates'] == 1 and expected['actual_child_commands'] == 0
    assert len(expected['images']) == 1
    raw, facts = model.derive(state, rows)
    image = expected['images'][0]
    assert image['id'] == origin['id'] and facts == image['facts']
    assert raw == pins.check(image['image']).read_bytes()
    # The complete SOURCE/request reconstruction precedes every AFTER read.
    receipt, git = build.verify(store, pins, args.verify_git)
    assert receipt['expected_images_frozen_before_build'] == store.pin(SOURCE + '/model-freeze.json')
    assert receipt['expected_report_frozen_before_build'] == store.pin(EXPECTED + '/report.json')
    public_plan = store.load(REFERENCE + '/plan.json')
    assert public_plan['commands'] == plan['future_reference_commands']
    public = report(store, pins, REFERENCE_WORK + '/report.json', 2)
    assert public['source_gates'] == len(public['proofs']) == len(public['images']) == 1
    for actual, declared in zip(public['commands'], public_plan['commands'], strict=True):
        assert actual['argv'] == declared['argv'] and actual['label'] == declared['label']
        assert pins.check(actual['stderr']).read_bytes() == b''
    assert pins.check(public['commands'][0]['stdout']).read_bytes() == b'APPENDED\tV2019-extended\t6\n'
    public_path = pins.check(public['images'][0])
    observed = reference.compare(state, public_path, pins.check(public['commands'][1]['stdout']), rows)
    assert json.loads(json.dumps(observed)) == public['proofs'][0]['facts']
    public_raw = public_path.read_bytes()
    difference = [position for position, (left, right) in enumerate(zip(raw, public_raw)) if left != right]
    assert len(public_raw) == len(raw) and len(difference) == 42
    null = next(row for row in observed['new_rows'] if row['identity'] == 12)
    frame = source.native.long_wire.frame(public_raw, null['page'], null['slot'])
    assert public_raw[frame['end'] - 1] == 1
    group_start, stamp_start, end = frame['start'] + 6, frame['start'] + 10, frame['start'] + 52
    assert all(group_start <= position < end for position in difference)
    assert sum(position < stamp_start for position in difference) == 1
    assert sum(position >= stamp_start for position in difference) == 41
    native = report(store, pins, args.producer_report, 1)
    getters = report(store, pins, args.getter_report, 1)
    runtime = store.load(NATIVE + '/plan.json')
    binding = store.load(NATIVE + '/getter-binding.json')
    assert binding['native_report'] == store.pin(args.producer_report)
    assert runtime['artifact'] == receipt['artifact']
    assert runtime['expected'] == image['image']
    assert runtime['logical_requests'] == rows
    assert runtime['classes'] == captured['classes']
    assert runtime['java_prefix'] == plan['source_commands'][2]['argv'][:-2]
    assert runtime['native_command']['argv'] == [runtime['artifact']['path'], origin['source'],
                                                runtime['output'], 'Items', SOURCE + '/requests.tsv']
    assert native['commands'][0]['argv'] == runtime['native_command']['argv']
    assert getters['commands'][0]['argv'] == runtime['getter_command']['argv']
    assert pins.check(native['commands'][0]['stdout']).read_bytes() == b'candidate_changed_rows=6\n'
    assert pins.check(native['commands'][0]['stderr']).read_bytes() == NOTICE
    assert pins.check(getters['commands'][0]['stderr']).read_bytes() == b''
    assert len(native['whole_images']) == len(getters['proofs']) == 1
    candidate = pins.check(native['whole_images'][0]['image'])
    assert candidate == store.path(runtime['output']) and candidate.read_bytes() == raw
    actual = reference.compare(state, candidate, pins.check(getters['commands'][0]['stdout']), rows)
    assert json.loads(json.dumps(actual)) == getters['proofs'][0]['facts']
    assert candidate.read_bytes() == raw
    assert actual['new_NULL_stamp_payloads']['12'] == bytes(42).hex()
    pins.terminal()
    return dict(verified=True, available=True, source_capture_commands=3, saved_source_gates=1,
                source_request_images_before_AFTER=1, public_reference_commands=2,
                public_reference_proofs=1, native_calls=1, fresh_combined_getter_map_calls=1,
                all_runtime_phases_first_attempt=True, preparation_history=history,
                public_differing_bytes=42, inactive_Group_differing_bytes=1,
                inactive_Stamp_differing_bytes=41, all_other_public_bytes_equal=True,
                expected=image['image'], build_artifact=receipt['artifact'], Git=git,
                pin_occurrences=pins.occurrences, distinct_paths=len(pins.items),
                actual_native_JVM_Cargo_calls_by_verifier=0, facts=actual)


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
