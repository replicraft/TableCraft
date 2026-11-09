"""Verify saved genuine Int32 evidence without running any fixture process."""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from compat_artifacts import ArtifactStore
from preserved_int32_genuine import framing, model

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RECORD = 'docs/testing/results/ace16-preserved-int32-genuine-images-2026-10-30.json'


def verify(root, record_path):
    store = ArtifactStore(root)
    checked = set()

    def check(pin):
        expected = {key: pin[key] for key in ['path', 'bytes', 'sha256']}
        identity = (pin['path'], pin['sha256'])
        framing.require(store.pin(pin['path']) == expected, 'Saved pin changed: ' + pin['path'])
        checked.add(identity)

    def load(pin):
        check(pin)
        return store.load(pin['path'])

    def streams(call):
        framing.require(call['started'] and call['returncode'] == 0
                        and call['capture_error'] is None, 'Successful saved call')
        for pin in call['logs']:
            check(pin)
        stdout, stderr = [store.path(pin['path']).read_bytes() for pin in call['logs']]
        framing.require(not stderr, 'Saved stderr must be empty')
        return stdout

    record = store.load(record_path)
    for pin in record['maintained_files']:
        check(pin)
    evidence = record['evidence']
    saved = {name: load(pin) for name, pin in evidence.items() if pin['path'].endswith('.json')}
    for name, pin in evidence.items():
        if name not in saved:
            check(pin)
    expected = saved['expectations']
    cases = expected['cases']
    report = saved['expected_report']
    framing.require(report['completed'] and report['source_gates'] == 2
                    and report['expected_images'] == 10
                    and report['children'] == report['writers'] == 0
                    and report['cases'] == cases, 'Completed pure expected construction')
    framing.require(len(cases) == 10, 'Exact ordered ten-case inventory')
    for pin in expected['inputs']:
        check(pin)
    for name in ['source_plan', 'native_plan', 'sdk_plan']:
        for pin in saved[name]['inputs']:
            check(pin)
    source = saved['source_receipt']
    framing.require(source['completed'] and source['failure'] is None
                    and source['native_or_cli_producer_calls'] == 0, 'Original-only SOURCE phase')
    framing.require([source[key] for key in ['planned_children', 'started_children', 'completed_children']]
                    == [5, 5, 5], 'Five saved original commands')
    framing.require(source['retained_originals'] == expected['sources'],
                    'Original source identities remain bound')
    for pin in source['retained_originals'] + source['retained_classes']:
        check(pin)
    framing.require([call['argv'] for call in source['calls']]
                    == [command['argv'] for command in saved['source_plan']['commands']],
                    'Original SOURCE calls match their prebound plan')
    for call in source['calls']:
        streams(call)
    export = saved['export_receipt']
    build = saved['build_receipt']
    framing.require(export['revision'] == record['producer_revision']
                    and export['all_files_match_git_objects']
                    and export['target_initially_absent']
                    and export['producer_invocations'] == 0, 'Committed export provenance')
    check(export['source_archive'])
    check(export['source_inventory'])
    framing.require(build['completed'] and build['exit_code'] == 0
                    and build['producer_invocations'] == 0
                    and build['source_inventory_unchanged_after_build']
                    and build['owning_engine_adapter_cli_compilation_observed'], 'Saved isolated CLI build')
    check(build['cli_artifact'])
    native = saved['native_receipt']
    sdk = saved['sdk_receipt']
    framing.require(native['completed'] and native['failure'] is None
                    and native['reference_reader_commands'] == 0, 'Native-only execution phase')
    framing.require([native[key] for key in ['planned_commands', 'attempted_commands', 'completed_commands']]
                    == [10, 10, 10], 'Ten saved native calls')
    framing.require(sdk['completed'] and sdk['failure'] is None
                    and sdk['native_or_cli_producer_calls'] == 0, 'Read-only SDK phase')
    framing.require([sdk[key] for key in ['planned_children', 'started_children', 'completed_children']]
                    == [10, 10, 10], 'Ten saved SDK reads')
    framing.require(len(native['calls']) == len(sdk['calls']) == 10, 'Exact saved call arrays')
    authorization = saved['native_authorization']
    framing.require([case['id'] for case in authorization['cases']]
                    == [case['id'] for case in cases], 'Ordered native authorization cases')
    recorded_root = Path(native['calls'][0]['argv'][0])
    for _ in Path(build['cli_artifact']['path']).parts:
        recorded_root = recorded_root.parent
    reconstructed = {}
    changed = 0
    for index, (case, call, reader) in enumerate(zip(cases, native['calls'], sdk['calls'], strict=True)):
        authorized = authorization['cases'][index]
        framing.require(all(authorized[key] == case[key]
                            for key in ['source', 'sql', 'native_request', 'expected', 'proof']),
                        'Exact original/request/expected authorization')
        for key in ['source', 'expected', 'proof', 'expected_reader_stdout']:
            check(case[key])
        source_path = case['source']['path']
        original_bytes = store.path(source_path).read_bytes()
        original_call = next(item for item in source['calls']
                             if item['id'] == f'inspect-h{original_bytes[20]}')
        if source_path not in reconstructed:
            original = store.path(source_path).read_bytes()
            metadata = framing.sdk_inventory(streams(original_call).decode('utf-8'))
            reconstructed[source_path] = (original, framing.qualify(original, metadata, original[20]))
        original, qualified = reconstructed[source_path]
        image, proof, intent, reader_text = model.construct(original, qualified, case['native_request'])
        framing.require(intent == case['native_request'], 'Original native request identity')
        actual_proof = load(case['proof'])
        framing.require(all(actual_proof[key] == value for key, value in proof.items()),
                        'Independent original/request framing proof')
        framing.require(image == store.path(case['expected']['path']).read_bytes(), 'Whole expected image')
        check(call['candidate'])
        candidate_bytes = store.path(call['candidate']['path']).read_bytes()
        framing.require(call['complete_image_equal'] and image == candidate_bytes,
                        'Whole actual native image')
        framing.require(call['argv'] == saved['native_plan']['commands'][index]
                        and call['argv'][3] == case['sql'], 'Exact saved native argv/literal SQL')
        framing.require(Path(call['argv'][0]) == recorded_root / build['cli_artifact']['path']
                        and call['argv'][4] == '--database'
                        and Path(call['argv'][5]) == recorded_root / case['source']['path']
                        and call['argv'][6] == '--output'
                        and Path(call['argv'][7]) == recorded_root / call['candidate']['path'],
                        'Exact recorded binary/source/output identity')
        count = int(case['native_request']['replacement']['value'] != 1000)
        changed += count
        stdout = ('Engineering copy\tExperimental; data macros and native Access saves unqualified\n'
                  'Matched rows\t1\n' + f'Candidate rows\t{count}\n').encode('utf-8')
        framing.require(streams(call) == stdout, 'Exact matched/changed CLI completion report')
        framing.require(reader['id'] == case['id'] and reader['whole_inventory_equal']
                        and reader['argv'] == saved['sdk_plan']['commands'][index], 'Exact saved SDK call')
        framing.require(reader['argv'] == original_call['argv'][:-1] + [call['argv'][7]],
                        'SDK uses exact original reader flags/classpath/class')
        framing.require(reader['argv'][-2] == '--inspect'
                        and Path(reader['argv'][-1]) == recorded_root / call['candidate']['path'],
                        'SDK reads the actual pinned native candidate')
        framing.require(streams(reader) == reader_text
                        == store.path(case['expected_reader_stdout']['path']).read_bytes(),
                        'Whole SDK inventory: selected Score only')
        check(saved['sdk_plan']['expected_observations'][index])
        observation_path = saved['sdk_plan']['expected_observations'][index]['path']
        framing.require(reader_text == store.path(observation_path).read_bytes(),
                        'SDK prebound expected observations')
    framing.require(len(reconstructed) == 2 and changed == 8, 'Two originals/eight changes/two noops')
    for pin in expected['sources']:
        check(pin)
    return {'saved_only': True, 'pins': len(checked), 'originals': 2, 'images': 10,
            'changed_cases': 8, 'noops': 2, 'sdk_reads': 10, 'child_processes': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default=str(ROOT))
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.record), sort_keys=True))


if __name__ == '__main__':
    main()
