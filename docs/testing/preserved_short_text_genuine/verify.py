"""Verify saved genuine Short Text evidence without invoking any fixture process."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from compat_artifacts import ArtifactStore
from preserved_short_text_genuine import prepare
from preserved_short_text_genuine.framing import require

ROOT = Path(__file__).resolve().parents[3]
DEFAULT_RECORD = 'docs/testing/results/ace16-preserved-short-text-genuine-images-2026-10-30.json'
REPORT_HEADER = b'Engineering copy\tExperimental; data macros and native Access saves unqualified\n'


def verify(root, record_path):
    store = ArtifactStore(root)
    checked = set()

    def check(pin):
        expected = {key: pin[key] for key in ['path', 'bytes', 'sha256']}
        require(store.pin(pin['path']) == expected, 'Saved artifact changed: ' + pin['path'])
        checked.add((pin['path'], pin['sha256']))

    def read(pin):
        check(pin)
        data = store.path(pin['path']).read_bytes()
        require(len(data) == pin['bytes'] and hashlib.sha256(data).hexdigest() == pin['sha256'],
                'Actual read matches saved identity')
        return data

    def load(pin):
        return json.loads(read(pin).decode('utf-8'))

    def streams(call, status=0):
        require(call['started'] and call['returncode'] == status
                and call['capture_error'] is None, 'Saved process outcome')
        require(len(call['logs']) == 2, 'Saved stdout/stderr pair')
        return tuple(read(pin) for pin in call['logs'])

    record = store.load(record_path)
    for pin in record['maintained_files']:
        check(pin)
    evidence = record['evidence']
    saved = {name: load(pin) for name, pin in evidence.items() if pin['path'].endswith('.json')}
    for name, pin in evidence.items():
        if name not in saved:
            check(pin)
    expected = saved['expectations']
    requests = saved['requests']
    cases = expected['cases']
    report = saved['expected_report']
    require(report['completed'] and report['failure'] is None and report['source_gates'] == 2
            and report['completed_cases'] == 22 and report['expected_images'] == 18
            and report['refusal_controls'] == 4 and report['child_calls'] == report['writer_calls'] == 0,
            'Completed independent pure construction')
    require(len(cases) == len(requests['cases']) == 22, 'Finite ordered literal cases')
    require(expected['request_inventory'] == evidence['requests'], 'Original request inventory binding')
    for pin in expected['inputs']:
        check(pin)
    for name in ['source_plan', 'native_plan', 'sdk_plan']:
        for pin in saved[name]['inputs']:
            check(pin)

    source = saved['source_receipt']
    require(source['completed'] and source['failure'] is None
            and source['native_or_cli_producer_calls'] == 0
            and [source[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [5, 5, 5], 'Five original-only source calls')
    require(len(source['calls']) == 5 and [call['argv'] for call in source['calls']]
            == [command['argv'] for command in saved['source_plan']['commands']], 'Exact source argv')
    for call in source['calls']:
        _, stderr = streams(call)
        require(not stderr, 'Original source stderr empty')
    for pin in source['retained_classes'] + source['retained_originals']:
        check(pin)
    originals = {}
    for pin in source['retained_originals']:
        raw = read(pin)
        call = next(call for call in source['calls'] if call['id'] == f'inspect-h{raw[20]}')
        require(Path(call['argv'][-1]).as_posix() == pin['path'], 'Getter reads exact original')
        getter, _ = streams(call)
        originals[raw[20]] = (pin, raw, getter.decode('utf-8'), call)
    require(set(originals) == {5, 6}, 'Both original source headers')

    export = saved['export_receipt']
    build = saved['build_receipt']
    require(export['revision'] == record['producer_revision'] and export['all_files_match_git_objects']
            and export['target_initially_absent'] and export['producer_invocations'] == 0,
            'Qualified committed export')
    check(export['source_archive'])
    check(export['source_inventory'])
    require(build['completed'] and build['exit_code'] == 0 and build['producer_invocations'] == 0
            and build['source_inventory_unchanged_after_build']
            and build['owning_engine_adapter_cli_compilation_observed'], 'Qualified isolated CLI build')
    check(build['cli_artifact'])
    build_plan = load(build['build_plan'])
    require(build_plan['revision'] == record['producer_revision'], 'Exact producer revision')
    native = saved['native_receipt']
    sdk = saved['sdk_receipt']
    require(native['completed'] and native['failure'] is None
            and [native[key] for key in ['planned_commands', 'started_commands', 'completed_commands']]
            == [22, 22, 22] and (native['successes'], native['refusals']) == (18, 4),
            'All saved native outcomes')
    require(sdk['completed'] and sdk['failure'] is None and sdk['native_or_cli_producer_calls'] == 0
            and [sdk[key] for key in ['planned_children', 'started_children', 'completed_children']]
            == [18, 18, 18], 'All saved readonly SDK calls')
    require(len(native['calls']) == len(saved['native_plan']['jobs']) == 22
            and len(sdk['calls']) == len(saved['sdk_plan']['jobs']) == 18, 'Exact saved call arrays')
    authorization = load(saved['native_plan']['manifest'])
    require(authorization['cases'] == cases
            and authorization['producer_revision'] == record['producer_revision']
            and authorization['independent_expectations'] == evidence['expectations'],
            'Exact original/request/expected authorization')
    historical_root = Path(native['calls'][0]['argv'][0])
    for _ in Path(build['cli_artifact']['path']).parts:
        historical_root = historical_root.parent
    output_root = Path(evidence['native_plan']['path']).parent
    def derive_all():
        for header in [5, 6]:
            source_cases = [case for case in requests['cases'] if case['header'] == header]
            yield from prepare.derive_cases(
                originals[header][1], originals[header][2], header, source_cases)

    reader_index = 0
    changed = 0
    comparisons = zip(requests['cases'], cases, native['calls'], derive_all(), strict=True)
    for index, (literal, case, call, derivation) in enumerate(comparisons):
        identifier, image, proof, reader_text = derivation
        require(identifier == literal['id'] == case['id'] == call['id'], 'Ordered exact case identity')
        require(all(case[key] == literal[key] for key in ['header', 'source', 'sql', 'native_request']),
                'Exact frozen source/SQL/native intent')
        require(case['expected_outcome'] == proof['expected_outcome'], 'Independently derived outcome')
        actual_proof = load(case['proof'])
        require(all(actual_proof[key] == value for key, value in proof.items()),
                'Independent complete record proof')
        relative_output = output_root / case['id'] / 'candidate.accdb'
        expected_argv = [str(historical_root / build['cli_artifact']['path']), 'query', 'preview-update',
                         case['sql'], '--database', str(historical_root / case['source']['path']),
                         '--output', str(historical_root / relative_output)]
        for flag, key in [('--work-bytes', 'bytes'), ('--work-rows', 'rows'), ('--work-steps', 'steps')]:
            expected_argv.extend([flag, str(authorization['work_limits'][key])])
        require(call['id'] == saved['native_plan']['jobs'][index]['id']
                and call['argv'] == expected_argv == saved['native_plan']['jobs'][index]['argv'],
                'Exact literal native command/binary/source/output')
        if image is None:
            stdout, stderr = streams(call, 2)
            require(case['expected'] is None and case['reader'] is None and call['candidate'] is None
                    and not stdout and b'native write profile is unsupported (RowLayout)' in stderr,
                    'Typed capacity refusal without success report')
            output = store.path(relative_output)
            require(not output.exists() and not output.is_symlink(), 'No refusal candidate artifact')
            continue
        require(read(case['expected']) == image and call['complete_image_equal']
                and call['candidate']['path'] == relative_output.as_posix()
                and read(call['candidate']) == image, 'Whole independently derived native image/EOF')
        count = proof['expected_outcome']['changed_rows']
        changed += count
        stdout, stderr = streams(call)
        require(not stderr and stdout == REPORT_HEADER + b'Matched rows\t1\nCandidate rows\t'
                + str(count).encode('ascii') + b'\n', 'Exact matched/changed completion report')
        require(actual_proof['expected_reader_stdout'] == case['reader']
                and read(case['reader']) == reader_text, 'Original-only complete SDK projection')
        reader = sdk['calls'][reader_index]
        job = saved['sdk_plan']['jobs'][reader_index]
        original_call = originals[case['header']][3]
        require(reader['id'] == job['id'] == case['id'] and reader['whole_inventory_equal']
                and reader['argv'] == job['argv'] == original_call['argv'][:-1] + [call['argv'][7]]
                and job['expected_reader_sha256'] == hashlib.sha256(reader_text).hexdigest(),
                'Exact unchanged original getter and actual candidate binding')
        stdout, stderr = streams(reader)
        require(not stderr and stdout == reader_text, 'Whole SDK inventory equals independent projection')
        reader_index += 1
    require(reader_index == 18 and changed == 14, 'Eighteen images: fourteen changes/four noops')
    for pin in source['retained_originals']:
        check(pin)
    return {'saved_only': True, 'pins': len(checked), 'originals': 2, 'native_calls': 22,
            'images': 18, 'capacity_refusals': 4, 'changed_cases': 14, 'noops': 4,
            'sdk_reads': 18, 'child_processes': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', default=str(ROOT))
    parser.add_argument('--record', default=DEFAULT_RECORD)
    args = parser.parse_args()
    print(json.dumps(verify(args.root, args.record), sort_keys=True))


if __name__ == '__main__':
    main()
