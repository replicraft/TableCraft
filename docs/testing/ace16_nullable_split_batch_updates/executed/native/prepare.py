"""Bind twenty original-row command lines before any native candidate call."""
import argparse
import json
from pathlib import Path
import subprocess

import bounded

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCE = ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029'
REFERENCE = ROOT / '.cache/ace16-nullable-split-batch-reference-next-20261029'
EXPECTED = ROOT / '.cache/ace16-nullable-split-batch-expected-first-20261029'
CONSTRUCTOR = ROOT / '.cache/ace16-nullable-split-batch-expected-next-20261029'
NATIVE_WORK = '.cache/ace16-nullable-split-batch-native-first-20261029'
GETTER_WORK = '.cache/ace16-nullable-split-batch-getters-first-20261029'
RECEIPT = 'docs/testing/results/ace16-nullable-split-batch-example-build-2026-10-29.json'


def assignments(case, source_case):
    rows = source_case['facts']['tables'][case['table']]['rows']
    by_id = {row['id']: (ordinal, row) for ordinal, row in enumerate(rows)}
    assert len(by_id) == 500
    result = []
    seen = set()
    for identity, values in case['updates']:
        assert identity in by_id and identity not in seen
        seen.add(identity)
        ordinal, row = by_id[identity]
        assert set(values) == {'Group', 'Score'}
        for column in ('Group', 'Score'):
            value = values[column]
            assert value is None or type(value) is int and -(1 << 31) <= value < 1 << 31
            result.append({'table': case['table'], 'column': column, 'ordinal': ordinal,
                           'identity': identity, 'logical_reference': row['root'],
                           'expected': row['values'][column], 'value': value})
    return result


def scalar(value):
    return 'null' if value is None else str(value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receipt-commit', required=True)
    args = parser.parse_args()
    receipt = json.loads((ROOT / RECEIPT).read_text())
    assert receipt['commit'] == '50d028cc19452c54046b1e1d23baa77ac80f491b'
    assert receipt['status'] == 0 and receipt['owning_engine_compilation_observed']
    assert receipt['fresh_target_was_absent'] and receipt['source_inventory_unchanged_after_build']
    artifact = receipt['artifacts']['batch']
    bounded.require_pin(artifact)
    raw_receipt = (ROOT / RECEIPT).read_bytes()
    committed = subprocess.check_output(['git', 'show', args.receipt_commit + ':' + RECEIPT], cwd=ROOT)
    assert committed == raw_receipt
    blob = subprocess.check_output(['git', 'rev-parse', args.receipt_commit + ':' + RECEIPT], cwd=ROOT).decode().strip()
    gates = json.loads((SOURCE / 'source-gates-first.json').read_text())
    assert gates['source_qualification'] and gates['source_gates'] == 8 and gates['sentinel_gates'] == 4
    sources = {case['id']: case for case in gates['facts']}
    requests = json.loads((CONSTRUCTOR / 'requests.json').read_text())
    expected_report = json.loads((EXPECTED / 'report.json').read_text())
    assert expected_report['completed'] and expected_report['model_derivations'] == 20
    images = {case['case']: case for case in expected_report['results']}
    reference = json.loads((REFERENCE / 'reference-plan.json').read_text())
    public_cases = {case['id']: case for case in reference['cases']}
    public_report_path = ROOT / '.cache/ace16-nullable-split-batch-reference-first-20261029/report.json'
    public_report = json.loads(public_report_path.read_text())
    assert public_report['completed'] and not public_report['pin_errors']
    assert len(public_report['commands']) == 40 and len(public_report['proofs']) == 20
    inspect_commands = [command for command in public_report['commands'] if command['label'].endswith('-inspect')]
    assert len(inspect_commands) == 20
    cases = []
    for index, case in enumerate(requests['cases']):
        source = sources[case['source_id']]
        original = public_cases[case['id']]
        image = images[case['id']]
        details = json.loads(bounded.path(image['details']['path']).read_text())
        fields = assignments(case, source)
        assert fields == details['assignments']
        changed = sorted({field['logical_reference'] for field in fields
                          if field['expected'] != field['value']})
        assert changed == details['changed_roots']
        output = NATIVE_WORK + '/' + case['id'] + '-native.accdb'
        argv = [artifact['path'], source['source']['path'], output, '--index-growth']
        for assignment in fields:
            argv.extend(['--set', assignment['table'], assignment['column'], str(assignment['ordinal']),
                         'int32', scalar(assignment['expected']), scalar(assignment['value'])])
        getter_argv = list(inspect_commands[index]['argv'])
        assert getter_argv[-3:] == ['inspect', original['output'], case['profile']]
        getter_argv[-2] = output
        proof_case = dict(original, output=output)
        cases.append(dict(id=case['id'], request=case, assignments=fields,
                          source=source['source'], before_getter=source['getter'],
                          expected=image['expected'], details=image['details'],
                          changed_rows=len(details['changed_roots']), output=output,
                          proof_case=proof_case,
                          native=dict(label=case['id'] + '-native', argv=argv),
                          getter=dict(label=case['id'] + '-getter', argv=getter_argv)))
    assert len(cases) == 20 and sum(len(case['request']['updates']) for case in cases) == 8008
    for directory in [ROOT / NATIVE_WORK, ROOT / GETTER_WORK]:
        assert not directory.exists()
    plan = dict(phase='nullable-split-batch-native20-before-fresh20', artifact=artifact,
                receipt=bounded.pin(RECEIPT), receipt_commit=args.receipt_commit, receipt_blob=blob,
                native_workdir=NATIVE_WORK, getter_workdir=GETTER_WORK, cases=cases,
                public_reference_report=bounded.pin(public_report_path),
                public_after_authority=False)
    classes_directory = ROOT / '.cache/ace16-nullable-split-batch-source-first-20261029/classes'
    plan['classes_directory'] = classes_directory.relative_to(ROOT).as_posix()
    plan['classes'] = [bounded.pin(path) for path in sorted(classes_directory.rglob('*.class'))]
    assert plan['classes']
    with (HERE / 'plan.json').open('x') as output:
        json.dump(plan, output, indent=2)
        output.write('\n')
    inputs = []
    for path in [SOURCE / 'source-gates-first.json', CONSTRUCTOR / 'freeze.json',
                 EXPECTED / 'freeze.json', REFERENCE / 'reference-freeze.json']:
        value = json.loads(path.read_text())
        inputs.extend(value.get('inputs', value.get('pins', [])))
        inputs.extend(value.get('outputs', []))
        inputs.append(bounded.pin(path))
    inputs.extend(public_report['inputs'])
    inputs.extend(public_report['artifacts'])
    inputs.extend([bounded.pin(public_report_path), artifact, bounded.pin(RECEIPT)])
    for path in [HERE / 'prepare.py', HERE / 'bounded.py', HERE / 'proof.py', HERE / 'capture.py',
                 HERE / 'bind_getters.py', HERE / 'plan.json', REFERENCE / 'reference.py',
                 REFERENCE / 'intent.py', REFERENCE / 'reference-plan.json']:
        inputs.append(bounded.pin(path))
    pins = {pin['path']: pin for pin in inputs}
    for pin in pins.values():
        bounded.require_pin(pin)
    with (HERE / 'freeze.json').open('x') as output:
        json.dump(dict(phase=plan['phase'], inputs=list(pins.values()), native_calls=20,
                       fresh_calls=20, native_before_fresh=True), output, indent=2)
        output.write('\n')
    print(json.dumps(dict(freeze=bounded.pin(HERE / 'freeze.json'), inputs=len(pins),
                          git_metadata_commands=2, native_commands=0, jvm_commands=0)))


if __name__ == '__main__':
    main()
