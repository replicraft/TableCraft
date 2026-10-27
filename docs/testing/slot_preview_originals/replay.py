"""Replay the fixed 162 original-span controls with a committed frozen consumer."""
import argparse
import csv
import hashlib
import io
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from slot_preview_originals import common, facts, model, proof

ROOT = common.ROOT


def parse(path):
    tables = facts.parse(common.path(path).read_text())
    for table in tables.values():
        for name in ['rows', 'raw_rows']:
            table[name] = {str(key): value for key, value in table[name].items()}
    return tables


def request_bytes(selection):
    stream = io.StringIO(newline='')
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    fields = [selection['ordinal']]
    for kind_name, data_name in [('expected_kind', 'expected_data_hex'),
                                 ('replacement_kind', 'replacement_data_hex')]:
        kind = selection[kind_name]
        data = bytes.fromhex(selection[data_name])
        text = data.decode('utf-16-le') if kind == 'text' else data.hex()
        fields.extend([kind, text])
    writer.writerow(fields)
    return stream.getvalue().encode('utf-8')


def validate_build(example, revision, receipt):
    assert receipt['compiled_revision'] == revision
    assert common.pin(example) == receipt['frozen_example']
    for source in receipt['source_pins']:
        data = subprocess.check_output(['git', 'show', revision + ':' + source['path']], cwd=ROOT)
        assert len(data) == source['bytes'] and hashlib.sha256(data).hexdigest() == source['sha256']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--reference-report', nargs=2, action='append', required=True,
                        metavar=('SERIES', 'PATH'))
    parser.add_argument('--verify-existing', type=Path,
                        help='Read-only verification of a captured replay; launches no candidate/getter')
    args = parser.parse_args()
    for name in [
        'example',
        'build_receipt',
        'workdir',
        'report',
        'jar',
        'classes',
        'plan',
        'facts',
        'preparation',
    ]:
        path = (ROOT / getattr(args, name)).resolve()
        path.relative_to(ROOT)
        setattr(args, name, path)
    assert not args.workdir.exists() and not args.report.exists()
    args.workdir.relative_to(ROOT / '.cache')
    plan_path = args.plan
    plan = json.loads(plan_path.read_text())
    assert common.pin(plan_path)['sha256'] == args.plan_sha256
    assert common.pin(args.facts) == plan['original_facts']
    assert common.pin(args.example)['sha256'] == args.sha256
    receipt = json.loads(args.build_receipt.read_text())
    validate_build(args.example, args.compiled_revision, receipt)
    preparation = json.loads(args.preparation.read_text())
    for item in preparation['inputs'] + preparation['tools']:
        assert common.pin(ROOT / item['path']) == item
    assert common.pin(args.jar) in preparation['tools']
    assert common.pin(args.classes / 'SlotUpdateFixture.class') in preparation['tools']
    originals = {item['profile']: item for item in json.loads(args.facts.read_text())}
    report_paths = [(series, common.path(path)) for series, path in args.reference_report]
    assert {series for series, _ in report_paths} == {'initial', 'null-supplement'}
    references = {series: json.loads(path.read_text()) for series, path in report_paths}
    assert model.fixed_plan(json.loads(args.facts.read_text()), report_paths) == plan['cases']
    immutable = [ROOT / item['path'] for item in preparation['inputs'] + preparation['tools']]
    immutable.extend([plan_path, Path(model.__file__), Path(proof.__file__), Path(__file__),
                      Path(common.__file__), Path(facts.__file__),
                      ROOT / 'docs/testing/capture_delete_trees.py',
                      args.example, args.build_receipt, args.facts, args.preparation])
    immutable.extend(path for _, path in report_paths)
    report = {'scope': ('Original-span engineering model; public canonicalization and '
                        'contrary FEFF writes remain independent.'),
              'compiled_revision': args.compiled_revision, 'plan': common.pin(plan_path),
              'pins_before': [common.pin(path) for path in immutable], 'cases': []}
    args.workdir.mkdir()
    capture = common.Capture(args.workdir)
    java = common.java(args.jar, args.classes)
    if args.verify_existing:
        old_report = json.loads(common.path(args.verify_existing).read_text())
        assert old_report['compiled_revision'] == args.compiled_revision
        assert [case['selection'] for case in old_report['cases']] == plan['cases']
        results = []
        for item in old_report['cases']:
            selection = item['selection']
            original = originals[selection['profile']]
            source = ROOT / original['input']['path']
            assert common.pin(source) == original['input']
            assert common.pin(ROOT / item['request']['path']) == item['request']
            assert (ROOT / item['request']['path']).read_bytes() == request_bytes(selection)
            if selection['expected_candidate']:
                candidate = ROOT / item['candidate']['path']
                assert common.pin(candidate) == item['candidate']
                getter = item['getter_command']['stdout']
                assert common.pin(ROOT / getter['path']) == getter
                actual = parse(ROOT / getter['path'])
                reference_case = references[selection['series']]['cases'][selection['reference_position']]
                reference = parse(ROOT / reference_case['getter']['stdout']['path'])
                reference_file = ROOT / reference_case['created_copy']['path']
                assert common.pin(reference_file) == reference_case['created_copy']
                proof_result = proof.verify(source.read_bytes(), candidate.read_bytes(), original,
                                            actual, selection, reference, reference_file.read_bytes())
                assert proof_result == item['proof']
            else:
                assert item['candidate_command']['status'] not in [None, 0]
                assert item['produced_candidate'] is False
                assert not common.path(item['candidate_command']['argv'][2]).exists()
            results.append({'profile': selection['profile'], 'label': selection['label'], 'agrees': True})
        report['cases'] = results
        report['scope'] = ('Current public-wrapper read-only proof verification; '
                           'no candidate/getter rerun; historical execution remains separate.')
        report['historical_report'] = common.pin(common.path(args.verify_existing))
        report['pins_after'] = [common.pin(path) for path in immutable]
        assert report['pins_after'] == report['pins_before']
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print('Verified', len(results), 'historical controls without launching candidates')
        return 0
    for number, selection in enumerate(plan['cases']):
        profile = selection['profile']
        original = originals[profile]
        source = ROOT / original['input']['path']
        assert common.pin(source) == original['input']
        profile_label = common.safe_label(profile)
        case_label = common.safe_label(selection['label'])
        stem = f'{number:03}-{profile_label}-{case_label}'
        request = args.workdir / (stem + '.tsv')
        request.write_bytes(request_bytes(selection))
        output = args.workdir / (stem + '.accdb')
        item = {'selection': selection, 'request': common.pin(request)}
        report['cases'].append(item)
        try:
            item['candidate_command'] = capture.run([
                str(args.example), str(source), str(output), selection['table'], selection['column'], str(request),
            ], stem + '-candidate')
            command = item['candidate_command']
            produced = command['status'] == 0 and output.is_file()
            item['produced_candidate'] = produced
            item['admission_agrees'] = produced == selection['expected_candidate'] and not command['timed_out']
            if selection['expected_candidate']:
                assert produced, 'Expected qualified candidate was refused'
                expected_changed = int(selection['proof']['old_getter'] != selection['proof']['replacement_getter'])
                assert (ROOT / command['stdout']['path']).read_text() == f'candidate_changed_rows={expected_changed}\n'
                item['candidate_before_getter'] = common.pin(output)
                item['getter_command'] = capture.run(java + ['oracle', str(output)], stem + '-getter')
                getter_error = ROOT / item['getter_command']['stderr']['path']
                assert item['getter_command']['status'] == 0 and not getter_error.read_bytes()
                actual = parse(ROOT / item['getter_command']['stdout']['path'])
                reference_case = references[selection['series']]['cases'][selection['reference_position']]
                reference_getter = reference_case['getter']['stdout']
                assert common.pin(ROOT / reference_getter['path']) == reference_getter
                reference = parse(ROOT / reference_case['getter']['stdout']['path'])
                reference_file = ROOT / reference_case['created_copy']['path']
                assert common.pin(reference_file) == reference_case['created_copy']
                item['proof'] = proof.verify(source.read_bytes(), output.read_bytes(), original,
                                             actual, selection, reference, reference_file.read_bytes())
                assert common.pin(output) == item['candidate_before_getter']
            else:
                refused = command['status'] not in [None, 0] and not output.exists()
                assert refused, 'Expected atomic refusal produced output or timed out'
            assert common.pin(source) == original['input']
            item['source_unchanged'] = True
            item['agrees'] = True
        except Exception as error:
            item['agrees'] = False
            item['diagnostic'] = type(error).__name__ + ': ' + str(error)
        finally:
            if output.exists():
                item['candidate'] = common.pin(output)
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(stem, 'agrees', item['agrees'], item.get('diagnostic', ''), flush=True)
    report['pins_after'] = [common.pin(path) for path in immutable]
    report['immutable_sources_unchanged'] = report['pins_before'] == report['pins_after']
    report['controls'] = len(report['cases'])
    report['disagreements'] = sum(not item['agrees'] for item in report['cases'])
    report['successful_candidates'] = sum(item.get('produced_candidate', False) for item in report['cases'])
    report['commands'] = len(capture.commands)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(
        'TERMINAL', report['controls'], 'controls', report['successful_candidates'],
        'candidates', report['disagreements'], 'disagreements', flush=True,
    )
    return 0 if report['disagreements'] == 0 and report['immutable_sources_unchanged'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
