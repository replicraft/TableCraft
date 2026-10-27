"""Replay only the separately frozen eight variable-update original controls."""
import argparse
import csv
import hashlib
import io
import json
import shutil
import subprocess
import sys
import traceback
from pathlib import Path

from slot_preview_originals import common, facts
import capture_cross_page_updates as capture
import delete_reference_maps as MAP
from . import proof

ROOT = common.ROOT

EXPECTED_CASES = 8
TABLE_ROWS_OFFSET = 16
INDEX_HISTORY_OFFSET = 67
COUNTER_BYTES = 4


def verify_pin(pin):
    assert common.pin(pin['path']) == pin


def frozen_inputs(plan):
    pins = [plan['reference_receipt'], plan['raw_reference_report']]
    for case in plan['cases']:
        if 'path' in case['source']:
            pins.append(case['source'])
        pins.extend([case['public_reference'], case['reference_getter'], case['reference_maps']])
    for pin in pins:
        verify_pin(pin)
    return pins


def verify_build(args):
    receipt = json.loads(common.path(args.build_receipt).read_text())
    assert receipt['compiled_revision'] == args.revision
    assert receipt['exit_code'] == 0
    artifact = common.pin(args.artifact)
    assert artifact == receipt['frozen_example']
    assert artifact['sha256'] == args.artifact_sha256
    verified = []
    for pin in receipt['source_pins']:
        data = subprocess.run(['git', 'show', args.revision + ':' + pin['path']], cwd=ROOT,
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        assert len(data) == pin['bytes']
        assert hashlib.sha256(data).hexdigest() == pin['sha256']
        verified.append(pin)
    return {'receipt': common.pin(args.build_receipt), 'artifact': artifact,
            'compiled_revision': args.revision, 'git_source_objects': verified,
            'scope': receipt.get('source_pin_scope', 'selected source pins, not exhaustive compiler inputs')}


def request_bytes(case):
    target = io.StringIO(newline='')
    writer = csv.writer(target, delimiter='\t', lineterminator='\n')
    writer.writerow([case['ordinal'], case['expected_kind'], case['expected_hex'],
                     'binary', case['replacement_hex']])
    return target.getvalue().encode('utf-8')


def write_report(path, report):
    path.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n')


def run(args, plan):
    build = verify_build(args)
    assert common.pin(args.jar)['sha256'] == common.JAR_SHA256
    assert common.pin('docs/testing/SlotUpdateFixture.java')['sha256'] == common.PRODUCER_SHA256
    assert build['artifact'] == plan['artifact']
    assert args.revision == plan['compiled_revision']
    capture.TABLE = 'BinaryStorage'
    capture.COLUMN = 'Bytes'
    capture.JAR = common.path(args.jar).relative_to(ROOT)
    capture.CLASSES = common.path(args.classes).relative_to(ROOT)
    assert (common.path(args.classes) / 'SlotUpdateFixture.class').is_file()
    assert (common.path(args.classes) / 'SlotMapProbe.class').is_file()
    immutable = frozen_inputs(plan)
    tools = [Path(__file__), Path(proof.__file__), Path(capture.__file__), Path(common.__file__), Path(facts.__file__),
             ROOT / 'docs/testing/delete_reference_maps.py', common.path(args.jar),
             common.path(args.classes) / 'SlotUpdateFixture.class', common.path(args.classes) / 'SlotMapProbe.class']
    tool_pins = [common.pin(path) for path in tools]
    work = common.fresh_work(args.workdir)
    archive = work / 'precall-source'
    archive.mkdir()
    for index, path in enumerate(tools):
        if path.suffix == '.py':
            shutil.copyfile(path, archive / (str(index) + '-' + path.name))
    shutil.copyfile(common.path(args.plan), archive / 'plan.json')
    requests = {}
    for case in plan['cases']:
        path = work / (case['id'] + '.tsv')
        path.write_bytes(request_bytes(case))
        requests[case['id']] = common.pin(path)
    before_first = {'frozen_plan': common.pin(args.plan), 'build': build, 'inputs': immutable,
                    'tools': tool_pins, 'requests': requests,
                    'source_archive': [common.pin(path) for path in sorted(archive.iterdir())]}
    write_report(work / 'before-first-call.json', before_first)
    report = {'scope': 'separate eight engineering candidates; no native Access/save qualification',
              'before_first_call': common.pin(work / 'before-first-call.json'), 'cases': [],
              'expected_cases': EXPECTED_CASES, 'completed': 0, 'disagreements': 0}
    report_path = work / 'report.json'
    commands = common.Capture(work)
    grown = {}
    for case in plan['cases']:
        record = {'id': case['id'], 'planned': case, 'request': requests[case['id']], 'complete': False}
        report['cases'].append(record)
        write_report(report_path, report)
        output = None
        try:
            if case['action'] == 'grow':
                source = common.path(case['source']['path'])
                old = capture.capture(source, case['id'] + '-before', commands)
            else:
                parent = grown[case['source']['candidate_from']]
                source = common.path(parent['file']['path'])
                old = parent['capture']
                assert common.pin(source) == parent['file']
            record['actual_source'] = common.pin(source)
            assert record['actual_source']['path'] != case['public_reference']['path']
            output = work / (case['id'] + '-candidate.accdb')
            assert not output.exists()
            argv = [str(common.path(args.artifact)), str(source), str(output), case['table'], case['column'],
                    requests[case['id']]['path'], '--variable-updates']
            assert argv.count('--variable-updates') == 1
            result = commands.run(argv, case['id'] + '-native')
            record['native_command'] = result
            assert result['status'] == 0 and not result['timed_out'], 'native candidate process failed'
            assert output.is_file()
            assert Path(result['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
            expected_warning = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
            assert Path(result['stderr']['path']).read_bytes() == expected_warning
            new = capture.capture(output, case['id'] + '-after', commands)
            reference = facts.parse(common.path(case['reference_getter']['path']).read_text())
            proof.same_semantics(new[1], reference)
            comparison = capture.compare(old, new)
            assert comparison['map_changes'] == case['expected_maps_from_public_raw']
            checked = proof.prove(case, old[0], new[0], old[2], new[2], old[1])
            record.update({'candidate': common.pin(output), 'capture': new[2], 'comparison': comparison,
                           'proof': checked, 'complete': True, 'full_typed_reference_rows_and_index_orders_match': True})
            assert common.pin(source) == record['actual_source']
            if case['action'] == 'grow':
                grown[case['id']] = {'file': common.pin(output), 'capture': new}
            report['completed'] += 1
        except Exception as error:
            record['error'] = {'class': type(error).__name__, 'message': str(error), 'traceback': traceback.format_exc()}
            report['disagreements'] += 1
            if output is not None and output.is_file():
                record['failed_candidate_artifact'] = common.pin(output)
        if 'actual_source' in record:
            record['actual_source_after'] = common.pin(record['actual_source']['path'])
            assert record['actual_source_after'] == record['actual_source']
        write_report(report_path, report)
    for pin in immutable + tool_pins + list(requests.values()):
        verify_pin(pin)
    assert common.pin(args.artifact) == build['artifact']
    report.update({'immutable_inputs_tools_and_requests_unchanged': True, 'command_count': len(commands.commands),
                   'native_candidate_calls': sum('native_command' in case for case in report['cases']),
                   'commands': commands.commands})
    write_report(report_path, report)
    print(json.dumps({'report': common.pin(report_path), 'completed': report['completed'], 'disagreements': report['disagreements']}))
    return 0 if report['completed'] == EXPECTED_CASES and report['disagreements'] == 0 else 1


def verify_existing(args, plan):
    path = common.path(args.verify_existing)
    historical_pin = common.pin(path)
    assert historical_pin['sha256'] == args.verify_existing_sha256
    historical = json.loads(path.read_text())
    assert historical['completed'] == EXPECTED_CASES and historical['disagreements'] == 0
    assert historical['native_candidate_calls'] == EXPECTED_CASES
    before_path = common.path(historical['before_first_call']['path'])
    verify_pin(historical['before_first_call'])
    frozen = json.loads(before_path.read_text())
    assert frozen['frozen_plan'] == common.pin(args.plan)
    verify_pin(frozen['build']['artifact'])
    verify_pin(frozen['build']['receipt'])
    assert frozen['build']['artifact'] == plan['artifact']
    assert frozen['build']['compiled_revision'] == plan['compiled_revision']
    for source in frozen['build']['git_source_objects']:
        data = subprocess.check_output(['git', 'show', plan['compiled_revision'] + ':' + source['path']], cwd=ROOT)
        assert len(data) == source['bytes']
        assert hashlib.sha256(data).hexdigest() == source['sha256']
    for pin in frozen['inputs'] + frozen['tools'] + list(frozen['requests'].values()) + frozen['source_archive']:
        verify_pin(pin)
    assert [case['planned'] for case in historical['cases']] == plan['cases']
    for command in historical['commands']:
        assert command['status'] == 0 and not command['timed_out']
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
    by_id = {case['id']: case for case in historical['cases']}
    results = []
    for record in historical['cases']:
        case = record['planned']
        assert record['complete']
        verify_pin(record['candidate'])
        verify_pin(record['actual_source'])
        assert common.path(record['request']['path']).read_bytes() == request_bytes(case)
        if case['action'] == 'grow':
            assert record['actual_source'] == case['source']
            metadata = facts.parse(common.path(case['public_source_getter']['path']).read_text())
        else:
            parent = by_id[case['source']['candidate_from']]
            assert record['actual_source'] == parent['candidate']
            metadata = facts.parse(common.path(parent['capture']['getter']['stdout']['path']).read_text())
        before = common.path(record['actual_source']['path']).read_bytes()
        after = common.path(record['candidate']['path']).read_bytes()
        definition = metadata[case['table']]['definition'] * proof.PAGE_BYTES
        state_before = {
            'declared_rows': int.from_bytes(before[definition + TABLE_ROWS_OFFSET:definition + TABLE_ROWS_OFFSET + COUNTER_BYTES], 'little'),
            'historical_index_rows': int.from_bytes(before[definition + INDEX_HISTORY_OFFSET:definition + INDEX_HISTORY_OFFSET + COUNTER_BYTES], 'little'),
        }
        state_after = {
            'declared_rows': int.from_bytes(after[definition + TABLE_ROWS_OFFSET:definition + TABLE_ROWS_OFFSET + COUNTER_BYTES], 'little'),
            'historical_index_rows': int.from_bytes(after[definition + INDEX_HISTORY_OFFSET:definition + INDEX_HISTORY_OFFSET + COUNTER_BYTES], 'little'),
        }
        for field, value in state_after.items():
            assert record['capture'][field] == value
        map_lines = common.path(record['capture']['map_probe']['stdout']['path']).read_text().splitlines()
        map_text = '\n'.join(line for line in map_lines if not line.startswith('LOCATION\t'))
        parsed_maps = MAP.parse(map_text)
        for name, description in parsed_maps['maps'].items():
            decoded = MAP.decode_map(after, description, parsed_maps['page_bytes'], parsed_maps['page_count'])
            # Raw decoder page keys are integers; the historical JSON serializer
            # necessarily stored these object keys as strings.
            decoded['bit_locations'] = {
                str(page): location for page, location in decoded['bit_locations'].items()
            }
            assert decoded == record['capture']['maps'][name]
        actual = facts.parse(common.path(record['capture']['getter']['stdout']['path']).read_text())
        reference = facts.parse(common.path(case['reference_getter']['path']).read_text())
        proof.same_semantics(actual, reference)
        result = proof.prove(case, before, after, state_before, state_after, metadata)
        assert result == record['proof']
        results.append({'id': case['id'], 'whole_image_proof_matches': True})
    assert common.pin(path) == historical_pin
    output = common.path(args.workdir)
    output.relative_to(ROOT / '.cache')
    assert not output.exists()
    output.mkdir()
    report = {
        'scope': 'Current public-wrapper read-only verification; historical execution remains separate.',
        'historical_report': historical_pin,
        'cases': results,
        'native_candidate_calls': 0,
        'JVM_calls': 0,
        'public_sources': [common.pin(Path(__file__)), common.pin(Path(proof.__file__))],
    }
    write_report(output / 'verification.json', report)
    print(json.dumps({'verified': len(results), 'native_calls': 0, 'JVM_calls': 0,
                      'report': common.pin(output / 'verification.json')}))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--verify-existing-sha256')
    parser.add_argument('--verify-existing', type=Path, help='Read-only cached image/getter/map proof; no native or JVM calls')
    parser.add_argument('--execute', action='store_true', help='Only after coordinator supplies committed frozen artifact GO')
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--artifact-sha256')
    parser.add_argument('--revision')
    parser.add_argument('--build-receipt', type=Path)
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--classes', type=Path)
    args = parser.parse_args()
    assert common.pin(args.plan)['sha256'] == args.plan_sha256
    plan = json.loads(common.path(args.plan).read_text())
    assert len(plan['cases']) == plan['expected_candidates'] == EXPECTED_CASES
    frozen_inputs(plan)
    if args.verify_existing is not None:
        assert not args.execute
        if args.verify_existing_sha256 is None or args.workdir is None:
            parser.error('--verify-existing requires --verify-existing-sha256 and --workdir')
        return verify_existing(args, plan)
    if not args.execute:
        print('Frozen eight-case plan and all source/reference pins verified; no candidate or getter calls.')
        return 0
    for name in ('artifact', 'artifact_sha256', 'revision', 'build_receipt', 'workdir', 'jar', 'classes'):
        if getattr(args, name) is None:
            parser.error('--execute requires --' + name.replace('_', '-'))
    return run(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
