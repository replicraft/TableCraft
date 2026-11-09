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

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common, facts
import capture_cross_page_updates as capture
from variable_preview_originals import proof
import delete_reference_maps as maps

PLAN_SHA256 = '24e0bc6eb9af67b39976a4a8eb731402e6291216c0015c50687ffb578cc9a022'
EXPECTED_CASES = 8
ENGINEERING_NOTICE = (
    'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
)


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
    expected = case['expected_text'] if case['expected_kind'] == 'text' else ''
    writer.writerow([case['ordinal'], case['expected_kind'], expected, 'text', case['replacement_text']])
    return target.getvalue().encode('utf-8')


def write_report(path, report):
    path.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n')


def saved_capture(state):
    """Decode preserved raw getters/maps and image bytes without running tools."""
    verify_pin(state['file'])
    for command in [state['getter'], state['map_probe']]:
        assert command['status'] == 0 and not command['timed_out']
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
        assert not common.path(command['stderr']['path']).read_bytes()
    raw = common.path(state['file']['path']).read_bytes()
    metadata = facts.parse(common.path(state['getter']['stdout']['path']).read_text())
    lines = []
    location = None
    for line in common.path(state['map_probe']['stdout']['path']).read_text().splitlines():
        if line.startswith('LOCATION\t'):
            location = list(map(int, line.split('\t')[1:]))
        else:
            lines.append(line)
    assert location is not None and len(location) == 4
    parsed = maps.parse('\n'.join(lines))
    decoded_maps = {
        name: maps.decode_map(raw, description, parsed['page_bytes'], parsed['page_count'])
        for name, description in parsed['maps'].items()
    }
    root_page, root_slot, target_page, target_slot = location
    definition = metadata[capture.TABLE]['definition'] * capture.PAGE_BYTES
    rebuilt = {
        'file': common.pin(state['file']['path']),
        'file_pages': len(raw) // capture.PAGE_BYTES,
        'getter': state['getter'],
        'map_probe': state['map_probe'],
        'root': capture.frame(raw, root_page, root_slot),
        'target': capture.frame(raw, target_page, target_slot),
        'row_getter': metadata[capture.TABLE]['rows'][capture.IDENTITY]['values'],
        'declared_rows': capture.number(raw, definition + capture.TABLE_ROWS_OFFSET, 4),
        'historical_index_rows': capture.number(raw, definition + capture.INDEX_HISTORY_OFFSET, 4),
        'maps': decoded_maps,
        'owned_data_page_headers': {},
    }
    for page in decoded_maps['TableOwned']['file_members']:
        base = page * capture.PAGE_BYTES
        rebuilt['owned_data_page_headers'][str(page)] = {
            'free_bytes': capture.number(raw, base + capture.FREE_OFFSET),
            'physical_slots': capture.number(raw, base + capture.ROW_COUNT_OFFSET),
            'prefix_hex': raw[base:base + capture.SLOT_START].hex(),
        }
    # JSON object keys normalize integer bitmap positions to strings.
    assert json.loads(json.dumps(rebuilt)) == state
    return raw, metadata, rebuilt


def verify_existing(path, plan):
    report_pin = common.pin(path)
    report = json.loads(common.path(path).read_text())
    totals = (
        report['completed'], report['disagreements'],
        report['native_candidate_calls'], report['command_count'],
    )
    assert totals == (8, 0, 8, 32)
    verify_pin(report['before_first_call'])
    before = json.loads(common.path(report['before_first_call']['path']).read_text())
    assert before['frozen_plan']['sha256'] == PLAN_SHA256
    for pin in before['inputs'] + before['tools'] + before['source_archive'] + list(before['requests'].values()):
        verify_pin(pin)
    build = before['build']
    verify_pin(build['artifact'])
    verify_pin(build['receipt'])
    receipt = json.loads(common.path(build['receipt']['path']).read_text())
    assert receipt['compiled_revision'] == build['compiled_revision']
    assert receipt['source_pins'] == build['git_source_objects']
    assert receipt['frozen_example'] == build['artifact']
    grown = {}
    for case, record in zip(plan['cases'], report['cases'], strict=True):
        assert record['planned'] == case and record['id'] == case['id'] and record['complete']
        verify_pin(record['request'])
        assert common.path(record['request']['path']).read_bytes() == request_bytes(case)
        verify_pin(record['actual_source'])
        assert record['actual_source'] == record['actual_source_after']
        if case['action'] == 'grow':
            assert record['actual_source'] == case['source']
            raw_reference = json.loads(common.path(plan['raw_reference_report']['path']).read_text())
            reference_case = next(
                item for item in raw_reference['cases']
                if all(item['selection'][field] == case[field]
                       for field in ['profile', 'history', 'action'])
            )
            old = saved_capture(reference_case['before'])
        else:
            old = grown[case['source']['candidate_from']]
            assert record['actual_source'] == old[2]['file']
        assert record['actual_source']['path'] != case['public_reference']['path']
        native = record['native_command']
        assert native['status'] == 0 and not native['timed_out']
        verify_pin(native['stdout'])
        verify_pin(native['stderr'])
        assert common.path(native['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
        assert common.path(native['stderr']['path']).read_text() == ENGINEERING_NOTICE
        new = saved_capture(record['capture'])
        assert new[2]['file'] == record['candidate']
        reference = facts.parse(common.path(case['reference_getter']['path']).read_text())
        proof.same_semantics(new[1], reference)
        comparison = capture.compare(old, new)
        assert comparison == record['comparison']
        assert comparison['map_changes'] == case['expected_maps_from_public_raw']
        assert proof.prove(case, old[0], new[0], old[2], new[2], old[1]) == record['proof']
        if case['action'] == 'grow':
            grown[case['id']] = new
    for command in report['commands']:
        assert command['status'] == 0 and not command['timed_out']
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
    assert common.pin(path) == report_pin
    print(json.dumps({'verified_existing_report': report_pin, 'complete_proofs': 8,
                      'new_native_getter_or_map_calls': 0, 'public_wrapper_historical_execution': False}))
    return 0


def run(args, plan):
    build = verify_build(args)
    assert common.pin(args.jar)['sha256'] == common.JAR_SHA256
    assert common.pin('docs/testing/SlotUpdateFixture.java')['sha256'] == common.PRODUCER_SHA256
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
            result = commands.run(argv, case['id'] + '-native')
            record['native_command'] = result
            assert result['status'] == 0 and not result['timed_out'], 'native candidate process failed'
            assert output.is_file()
            assert Path(result['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
            new = capture.capture(output, case['id'] + '-after', commands)
            reference = facts.parse(common.path(case['reference_getter']['path']).read_text())
            proof.same_semantics(new[1], reference)
            comparison = capture.compare(old, new)
            assert comparison['map_changes'] == case['expected_maps_from_public_raw']
            checked = proof.prove(case, old[0], new[0], old[2], new[2], old[1])
            record.update({
                'candidate': common.pin(output), 'capture': new[2], 'comparison': comparison,
                'proof': checked, 'complete': True,
                'full_typed_reference_rows_and_index_orders_match': True,
            })
            assert common.pin(source) == record['actual_source']
            if case['action'] == 'grow':
                grown[case['id']] = {'file': common.pin(output), 'capture': new}
            report['completed'] += 1
        except Exception as error:
            record['error'] = {
                'class': type(error).__name__, 'message': str(error),
                'traceback': traceback.format_exc(),
            }
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
    print(json.dumps({
        'report': common.pin(report_path), 'completed': report['completed'],
        'disagreements': report['disagreements'],
    }))
    return 0 if report['completed'] == EXPECTED_CASES and report['disagreements'] == 0 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, default=Path('docs/testing/variable_preview_originals/plan.json'))
    parser.add_argument(
        '--verify-existing', type=Path,
        help='Recheck a preserved report without invoking native or Java tools',
    )
    parser.add_argument(
        '--execute', action='store_true',
        help='Only after coordinator supplies committed frozen artifact GO',
    )
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--artifact-sha256')
    parser.add_argument('--revision')
    parser.add_argument('--build-receipt', type=Path)
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--jar', type=Path)
    parser.add_argument('--classes', type=Path)
    args = parser.parse_args()
    assert common.pin(args.plan)['sha256'] == PLAN_SHA256
    plan = json.loads(common.path(args.plan).read_text())
    assert len(plan['cases']) == plan['expected_candidates'] == EXPECTED_CASES
    frozen_inputs(plan)
    if args.verify_existing is not None:
        if args.execute:
            parser.error('--verify-existing and --execute are mutually exclusive')
        return verify_existing(args.verify_existing, plan)
    if not args.execute:
        print('Frozen eight-case plan and all source/reference pins verified; no candidate or getter calls.')
        return 0
    for name in ('artifact', 'artifact_sha256', 'revision', 'build_receipt', 'workdir', 'jar', 'classes'):
        if getattr(args, name) is None:
            parser.error('--execute requires --' + name.replace('_', '-'))
    return run(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
