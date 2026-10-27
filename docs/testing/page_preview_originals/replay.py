"""Replay the immutable 180 PAGE selectors using an explicit frozen consumer."""
import argparse
import copy
import csv
import hashlib
import io
import json
import os
import subprocess
from pathlib import Path

from slot_preview_originals import common, facts, model as SLOT

from . import model as PAGE, proof as PROOF

ROOT = common.ROOT
SLOT_RECEIPT = Path('docs/testing/results/ace16-slot-original-candidates-2026-10-26.json')
WARNING = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def relative(path):
    path = (ROOT / path).resolve()
    return path.relative_to(ROOT)


def parse_getter(path):
    tables = facts.parse(Path(path).read_text())
    for table in tables.values():
        table['rows'] = {str(key): value for key, value in table['rows'].items()}
        table['raw_rows'] = {str(key): value for key, value in table['raw_rows'].items()}
    return tables


def request_bytes(selection):
    stream = io.StringIO(newline='')
    fields = [selection['ordinal']]
    for token in [selection['expected'], selection['replacement']]:
        kind, data = SLOT.value(token)
        encoded = data.decode('utf-16-le') if kind == 'text' else data.hex()
        fields.extend([kind, encoded])
    csv.writer(stream, delimiter='\t', lineterminator='\n').writerow(fields)
    return stream.getvalue().encode('utf-8')


def validate_build(example, revision, receipt_path, expected_hash):
    receipt = json.loads(receipt_path.read_text())
    assert len(revision) == 40 and all(character in '0123456789abcdef' for character in revision)
    assert receipt['compiled_revision'] == revision
    assert common.pin(example) == receipt['frozen_example']
    assert receipt['frozen_example']['sha256'] == expected_hash
    for source in receipt['source_pins']:
        data = subprocess.check_output(['git', 'show', revision + ':' + source['path']])
        assert len(data) == source['bytes']
        assert hashlib.sha256(data).hexdigest() == source['sha256']
    return receipt


def source_tables(original, selection):
    tables = copy.deepcopy(original['tables'])
    if selection['getter'] is None:
        return tables
    current = parse_getter(selection['getter']['path'])
    for name, table in tables.items():
        table['rows'] = current[name]['rows']
        table['raw_rows'] = current[name]['raw_rows']
        for label, index in table['indexes'].items():
            index['order'] = current[name]['indexes'][label]['order']
    return tables


def reference_for(selection, reports):
    if selection['series'] != 'Slot162':
        return None
    key = (selection['profile'], selection['label'])
    return reports[key]


def write_report(path, report):
    path.write_text(json.dumps(report, indent=2) + '\n')


def verify_existing(args, plan, originals, references, historical_refusals,
                    report, immutable, before_pins):
    historical_path = relative(args.verify_existing)
    historical_pin = common.pin(historical_path)
    historical = json.loads(historical_path.read_text())
    assert historical['compiled_revision'] == args.compiled_revision
    assert historical['artifact'] == common.pin(args.example)
    assert historical['plan'] == common.pin(args.plan)
    assert [case['selection'] for case in historical['cases']] == plan['selectors']
    assert historical['pins_before'] == historical['pins_after']
    for pin in historical['pins_before']:
        assert common.pin(pin['path']) == pin
    report['scope'] = 'Current public-wrapper read-only verification of the accepted private replay; no candidate or getter launches.'
    report['historical_report'] = common.pin(historical_path)
    report['cases'] = []
    for number, old in enumerate(historical['cases']):
        selection = old['selection']
        assert old['agrees'] and old['source_unchanged']
        for field in ['candidate_command', 'getter_command']:
            if field not in old:
                continue
            for stream in ['stdout', 'stderr']:
                pin = old[field][stream]
                assert common.pin(pin['path']) == pin
        assert old['attempted_command'].count('--page-updates') == 1
        assert common.pin(old['request']['path']) == old['request']
        assert Path(old['request']['path']).read_bytes() == request_bytes(selection)
        if selection['expected_candidate']:
            command = old['candidate_command']
            assert command['status'] == 0 and not command['timed_out']
            changed = int(selection['expected'] != selection['replacement'])
            assert Path(command['stdout']['path']).read_bytes() == f'candidate_changed_rows={changed}\n'.encode()
            assert Path(command['stderr']['path']).read_bytes() == WARNING
            assert old['getter_command']['status'] == 0 and not old['getter_command']['timed_out']
            assert Path(old['getter_command']['stderr']['path']).read_bytes() == b''
            assert common.pin(old['candidate']['path']) == old['candidate']
            assert common.pin(old['getter_command']['stdout']['path']) == old['getter_command']['stdout']
            actual = parse_getter(old['getter_command']['stdout']['path'])
            tables = source_tables(originals[selection['profile']], selection)
            reference_case = reference_for(selection, references)
            reference = None
            reference_raw = None
            if reference_case is not None:
                reference = parse_getter(reference_case['getter']['stdout']['path'])
                reference_raw = Path(reference_case['created_copy']['path']).read_bytes()
            result = PROOF.verify(
                Path(selection['input']['path']).read_bytes(),
                Path(old['candidate']['path']).read_bytes(), tables, actual,
                selection, reference, reference_raw,
            )
            assert result == old['proof']
        else:
            command = old['candidate_command']
            assert command['status'] not in [None, 0] and not command['timed_out']
            assert not Path(old['attempted_command'][2]).exists()
            original = historical_refusals[selection['profile'], selection['label']]
            assert Path(command['stdout']['path']).read_bytes() == b''
            assert Path(command['stderr']['path']).read_bytes() == Path(original['path']).read_bytes()
        report['cases'].append({'selector': number, 'agrees': True})
    report['pins_after'] = [common.pin(path) for path in sorted(immutable)]
    assert report['pins_after'] == before_pins
    assert common.pin(historical_path) == historical_pin
    report['historical_report_unchanged'] = True
    report['controls'] = len(report['cases'])
    report['accepted_candidates_verified'] = 156
    report['typed_refusals_verified'] = 24
    report['native_candidate_calls'] = 0
    report['public_getter_calls'] = 0
    write_report(args.report, report)
    print('Verified 180 controls, 156 candidate proofs and 24 typed refusals without launching candidates/getters')
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--initial-report', type=Path, required=True)
    parser.add_argument('--null-report', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--request-byte-pins', type=Path, required=True)
    parser.add_argument('--request-pins-sha256', required=True)
    parser.add_argument('--verify-existing', type=Path,
                        help='Read-only proof verification; launches no candidate or getter')
    args = parser.parse_args()
    for field in ['example', 'build_receipt', 'workdir', 'report', 'plan', 'facts',
                  'initial_report', 'null_report', 'jar', 'classes', 'request_byte_pins']:
        setattr(args, field, relative(getattr(args, field)))
    args.workdir.relative_to('.cache')
    args.report.relative_to('.cache')
    assert not args.workdir.exists() and not args.report.exists()
    assert common.pin(args.plan)['sha256'] == args.plan_sha256
    plan = json.loads(args.plan.read_text())
    assert plan['selector_count'] == len(plan['selectors']) == 180
    assert common.pin(args.request_byte_pins)['sha256'] == args.request_pins_sha256
    request_pins = json.loads(args.request_byte_pins.read_text())
    assert len(request_pins) == 180
    assert common.pin(args.jar)['sha256'] == common.JAR_SHA256
    assert common.pin(args.classes / 'SlotUpdateFixture.class') in plan['immutable_pins']
    for path in [args.facts, args.initial_report, args.null_report]:
        assert common.pin(path) in plan['immutable_pins']
    for item in plan['immutable_pins']:
        assert common.pin(item['path']) == item
    build = validate_build(args.example, args.compiled_revision, args.build_receipt, args.sha256)
    slot_receipt = json.loads(SLOT_RECEIPT.read_text())
    slot_raw_pin = slot_receipt['raw_report']
    assert common.pin(slot_raw_pin['path']) == slot_raw_pin
    slot_raw = json.loads(Path(slot_raw_pin['path']).read_text())
    historical_refusals = {}
    for case in slot_raw['cases']:
        old = case['selection']
        if old['expected_candidate']:
            continue
        historical_refusals[old['profile'], old['label']] = case['candidate_command']['stderr']
    original_facts = args.facts
    originals = {item['profile']: item for item in json.loads(original_facts.read_text())}
    references = {}
    reference_paths = [args.initial_report, args.null_report]
    for path in reference_paths:
        for case in json.loads(path.read_text())['cases']:
            label = case.get('case', case['table'] + '-nonempty-null')
            references[case['profile'], label] = case
    immutable = {Path(item['path']) for item in plan['immutable_pins']}
    immutable.update([args.example, args.build_receipt, SLOT_RECEIPT,
                      Path(slot_raw_pin['path']), Path(__file__).resolve().relative_to(ROOT),
                      args.plan, args.facts, args.initial_report, args.null_report,
                      Path(PROOF.__file__).resolve().relative_to(ROOT),
                      Path(PAGE.__file__).resolve().relative_to(ROOT),
                      args.request_byte_pins,
                      Path(common.__file__).resolve().relative_to(ROOT)])
    for selection in plan['selectors']:
        if not selection['expected_candidate']:
            old_stderr = historical_refusals[selection['profile'], selection['label']]
            assert common.pin(old_stderr['path']) == old_stderr
            immutable.add(Path(old_stderr['path']))
        reference = reference_for(selection, references)
        if reference is not None:
            immutable.add(Path(reference['created_copy']['path']))
            immutable.add(Path(reference['getter']['stdout']['path']))
    before_pins = [common.pin(path) for path in sorted(immutable)]
    args.workdir.mkdir()
    archive = args.workdir / 'pre-call-sources'
    archive.mkdir()
    source_archive = []
    for source in [Path(PAGE.__file__).resolve().relative_to(ROOT),
                   Path(PROOF.__file__).resolve().relative_to(ROOT),
                   Path(__file__).resolve().relative_to(ROOT)]:
        target = archive / source.name
        target.write_bytes(source.read_bytes())
        source_pin = common.pin(source)
        archive_pin = common.pin(target)
        assert source_pin['sha256'] == archive_pin['sha256']
        source_archive.append({'source': source_pin, 'archive': archive_pin})
    report = {
        'scope': 'Independent PAGE engineering comparison; Slot162 remains unchanged and contrary FEFF public writes remain refusals, not goldens.',
        'compiled_revision': args.compiled_revision,
        'artifact': common.pin(args.example),
        'build_receipt': common.pin(args.build_receipt),
        'git_source_pin_count': len(build['source_pins']),
        'plan': common.pin(args.plan),
        'pre_call_source_archive': source_archive,
        'pins_before': before_pins,
        'cases': [],
    }
    write_report(args.report, report)
    if args.verify_existing:
        return verify_existing(args, plan, originals, references, historical_refusals,
                               report, immutable, before_pins)
    capture = common.Capture(args.workdir)
    java = ['java', '-Dstdout.encoding=UTF-8', '-cp',
            os.pathsep.join([str(args.jar), str(args.classes)]), 'SlotUpdateFixture']
    for number, selection in enumerate(plan['selectors']):
        stem = f'{number:03}-{selection["profile"]}-{selection["series"]}-{selection["label"]}'
        request = args.workdir / (stem + '.tsv')
        request.write_bytes(request_bytes(selection))
        expected_request = request_pins[number]
        assert expected_request['profile'] == selection['profile']
        assert expected_request['series'] == selection['series']
        assert expected_request['label'] == selection['label']
        assert common.pin(request)['sha256'] == expected_request['sha256']
        assert request.stat().st_size == expected_request['bytes']
        output = args.workdir / (stem + '.accdb')
        command = [str(args.example), selection['input']['path'], str(output),
                   selection['table'], selection['column'], str(request), '--page-updates']
        assert command.count('--page-updates') == 1
        item = {'selection': selection, 'request': common.pin(request), 'attempted_command': command}
        report['cases'].append(item)
        write_report(args.report, report)
        try:
            assert common.pin(selection['input']['path']) == selection['input']
            item['candidate_command'] = capture.run(command, stem + '-candidate')
            receipt = item['candidate_command']
            produced = receipt['status'] == 0 and not receipt['timed_out'] and output.is_file()
            item['produced_candidate'] = produced
            item['created_output'] = output.exists()
            assert produced == selection['expected_candidate'], 'Admission disagrees with frozen plan'
            stdout = Path(receipt['stdout']['path']).read_bytes()
            stderr = Path(receipt['stderr']['path']).read_bytes()
            if selection['expected_candidate']:
                changed = int(selection['expected'] != selection['replacement'])
                assert stdout == f'candidate_changed_rows={changed}\n'.encode()
                assert stderr == WARNING
                before_getter = common.pin(output)
                item['candidate_before_getter'] = before_getter
                item['getter_command'] = capture.run(java + ['oracle', str(output)], stem + '-getter')
                getter = item['getter_command']
                assert getter['status'] == 0 and not getter['timed_out']
                assert Path(getter['stderr']['path']).read_bytes() == b''
                actual = parse_getter(getter['stdout']['path'])
                tables = source_tables(originals[selection['profile']], selection)
                reference_case = reference_for(selection, references)
                reference = None
                reference_raw = None
                if reference_case is not None:
                    reference = parse_getter(reference_case['getter']['stdout']['path'])
                    reference_raw = Path(reference_case['created_copy']['path']).read_bytes()
                item['proof'] = PROOF.verify(
                    Path(selection['input']['path']).read_bytes(), output.read_bytes(),
                    tables, actual, selection, reference, reference_raw,
                )
                assert common.pin(output) == before_getter
            else:
                assert receipt['status'] not in [None, 0] and not receipt['timed_out']
                assert not output.exists() and stdout == b''
                original_error = historical_refusals[selection['profile'], selection['label']]
                assert stderr == Path(original_error['path']).read_bytes()
                item['original_typed_refusal_exact'] = True
            assert common.pin(selection['input']['path']) == selection['input']
            item['source_unchanged'] = True
            item['agrees'] = True
        except Exception as error:
            item['agrees'] = False
            item['diagnostic'] = type(error).__name__ + ': ' + str(error)
        finally:
            if output.exists():
                item['candidate'] = common.pin(output)
            write_report(args.report, report)
        print(stem, 'agrees', item['agrees'], item.get('diagnostic', ''), flush=True)
    report['pins_after'] = [common.pin(path) for path in sorted(immutable)]
    report['immutable_unchanged'] = report['pins_after'] == before_pins
    report['controls'] = len(report['cases'])
    report['disagreements'] = sum(not item['agrees'] for item in report['cases'])
    report['candidates'] = sum(item.get('produced_candidate', False) for item in report['cases'])
    report['commands'] = len(capture.commands)
    write_report(args.report, report)
    print('TERMINAL', report['controls'], report['candidates'], report['disagreements'], flush=True)
    return 0 if report['disagreements'] == 0 and report['immutable_unchanged'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
