"""Explicitly gated two-candidate split replay; inert unless --execute is supplied."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import traceback

ROOT = Path(__file__).resolve().parents[3]
DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
from root_split_preview_originals import proof, verify_existing
from index_split_originals import capture

PLAN_SHA256 = 'ea06b8c3d1fd426e6680f24c4bb7ae5beac086a7e5b5f098e9b318d3dd0582db'
PROOF_SHA256 = 'd932823b6b96ce678cefce678f7fe7bb7b6d6aa8f687e7d7dbac2138952ade28'
JAR = '.cache/tablecraft-fixtures/jackcess-5.0.4.jar'
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
CONSUMER = 'crates/engines/tablecraft-ace/examples/preview_ace16_split.rs'


def verify(pin):
    assert capture.pin(pin['path']) == pin


def receipt(args):
    build = json.loads(capture.common.path(args.build_receipt).read_text())
    assert build['compiled_revision'] == args.revision
    assert build['exit_code'] == 0
    artifact = capture.pin(args.artifact)
    assert artifact == build['frozen_example']
    assert artifact['sha256'] == args.artifact_sha256
    found = False
    for item in build['source_pins']:
        command = ['git', 'show', args.revision + ':' + item['path']]
        process = subprocess.run(command, cwd=ROOT, capture_output=True, check=True, timeout=30)
        data = process.stdout
        assert len(data) == item['bytes']
        assert hashlib.sha256(data).hexdigest() == item['sha256']
        if item['path'] == CONSUMER:
            assert b'preview_ace16_split_int32_updates' in data
            found = True
    assert found, 'Build receipt must pin the actual split consumer'
    return {
        'receipt': capture.pin(args.build_receipt),
        'artifact': artifact,
        'compiled_revision': args.revision,
        'source_pins': build['source_pins'],
    }


def restored_state(state):
    result = copy.deepcopy(state)
    for name in ['getter', 'map_probe']:
        for stream in ['stdout', 'stderr']:
            verify(state[name][stream])
    metadata, relations = capture.parse(capture.common.path(state['getter']['stdout']['path']).read_text())
    result['metadata'] = metadata
    assert relations == state['relations']
    return result


def compare_reference(actual, reference):
    assert actual['relations'] == reference['relations']
    assert actual['metadata'].keys() == reference['metadata'].keys()
    for name, table in actual['metadata'].items():
        expected = reference['metadata'][name]
        assert table['geometry'] == expected['geometry']
        assert table['columns'] == expected['columns']
        assert table['rows'] == expected['rows']
        assert table['locations'] == expected['locations']
        assert table['indexes'] == expected['indexes']
        assert actual['map_info']['indexes'].keys() == reference['map_info']['indexes'].keys()
        for root, tree in actual['geometry'][name]['trees'].items():
            other = reference['geometry'][name]['trees'][root]
            assert tree['logical_names'] == other['logical_names']
            assert tree['entry_hex'] == other['entry_hex']
    return {'all_typed_rows_locations_logical_indexes_full_key_row_streams_agree': True,
            'public_Wide_redistribution_is_not_byte_golden': True}


def snapshot(directory, immutable, plan_path):
    archive = directory / 'executed-sources'
    archive.mkdir()
    sources = []
    for path in [DIRECTORY / 'replay.py', DIRECTORY / 'proof.py', plan_path]:
        before = capture.pin(path)
        target = archive / path.name
        shutil.copyfile(path, target)
        archived = capture.pin(target)
        assert archived['sha256'] == before['sha256']
        sources.append({'executed_source': before, 'archive': archived})
    record = {'sources': sources, 'immutable': immutable,
              'request_encoding': 'Seven positional consumer args; exact argv is the request, no TSV'}
    path = directory / 'before-first-call.json'
    path.write_text(json.dumps(record, indent=2) + '\n')
    return capture.pin(path)


def execute(args, plan):
    build = receipt(args)
    directory = capture.common.fresh_work(args.workdir)
    commands = capture.common.Capture(directory)
    dependencies = [
        'docs/testing/index_split_originals/capture.py',
        'docs/testing/capture_delete_trees.py',
        'docs/testing/delete_reference_maps.py',
        'docs/testing/slot_preview_originals/common.py',
        'docs/testing/BatchDeleteFixture.java',
        JAR,
    ]
    immutable = list(plan['immutable'])
    immutable.extend(capture.pin(path) for path in dependencies)
    immutable.extend([capture.pin(args.plan), capture.pin(DIRECTORY / 'proof.py'),
                      capture.pin(__file__), build['receipt'], build['artifact']])
    for item in immutable:
        verify(item)
    assert capture.pin(JAR)['sha256'] == capture.common.JAR_SHA256
    reference_report = json.loads(capture.common.path(plan['immutable'][0]['path']).read_text())
    for observed in reference_report['cases']:
        for state_name in ['before', 'after']:
            state = observed[state_name]
            for command_name in ['getter', 'map_probe']:
                immutable.extend([state[command_name]['stdout'], state[command_name]['stderr']])
    requests = {}
    for case in plan['cases']:
        candidate = directory / (case['profile'] + '-split-candidate.accdb')
        argv = [build['artifact']['path'], case['original']['path'], str(candidate.relative_to(ROOT)),
                *case['candidate_arguments_after_source_output']]
        request_path = directory / (case['profile'] + '-request.json')
        request_path.write_text(json.dumps({'argv': argv}, indent=2) + '\n')
        requests[case['profile']] = {'argv': argv, 'pin': capture.pin(request_path)}
        immutable.append(requests[case['profile']]['pin'])
    for item in immutable:
        verify(item)
    output = directory / 'report.json'
    report = {
        'scope': 'Two new split candidates with exact original-image proof and independent cached public-reference semantics',
        'build': build,
        'before_first_call': snapshot(directory, immutable, capture.common.path(args.plan)),
        'cases': [],
        'completed': 0,
        'fatal': None,
    }
    try:
        for case, public in zip(plan['cases'], reference_report['cases'], strict=True):
            candidate = directory / (case['profile'] + '-split-candidate.accdb')
            item = {'profile': case['profile'], 'selector': case, 'complete': False,
                    'candidate_path': str(candidate.relative_to(ROOT))}
            report['cases'].append(item)
            try:
                source = case['original']['path']
                request = requests[case['profile']]
                argv = request['argv']
                item['request'] = request['pin']
                original = capture.common.path(source).read_bytes()
                item['command'] = commands.run(argv, case['profile'] + '-candidate')
                item['candidate_exists'] = candidate.exists()
                if candidate.exists():
                    item['candidate'] = capture.pin(candidate)
                command = item['command']
                assert command['status'] == 0 and not command['timed_out']
                assert capture.common.path(command['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
                assert capture.common.path(command['stderr']['path']).read_text() == NOTICE
                actual = candidate.read_bytes()
                item['whole_image_proof'] = proof.prove(case, original, actual)
                _, state = capture.capture(candidate.relative_to(ROOT), case['profile'] + '-candidate', commands, JAR)
                item['getter_state'] = state
                reference_after = restored_state(public['after'])
                reference_before = restored_state(public['before'])
                item['reference_agreement'] = compare_reference(state, reference_after)
                comparison = capture.compare((original, reference_before), (actual, state))
                assert not comparison['public_rewritten_pages_with_unchanged_full_key_row_streams']
                assert all(counter['before'] == counter['after'] for counter in comparison['historical_counters_observed_not_assumed'])
                item['original_comparison'] = comparison
                verify(case['original'])
                item['complete'] = True
                report['completed'] += 1
            except Exception as error:
                item['error'] = {'class': type(error).__name__, 'message': str(error),
                                 'traceback': traceback.format_exc()}
            finally:
                if candidate.exists():
                    item['candidate'] = capture.pin(candidate)
                report['commands'] = commands.commands
                output.write_text(json.dumps(report, indent=2) + '\n')
        for item in immutable:
            verify(item)
        report['immutable_inputs_unchanged'] = True
    except Exception as error:
        report['fatal'] = {'class': type(error).__name__, 'message': str(error),
                           'traceback': traceback.format_exc()}
    report['commands'] = commands.commands
    report['command_count'] = len(commands.commands)
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': capture.pin(output), 'completed': report['completed'],
                      'command_count': report['command_count'], 'fatal': report['fatal']}))
    return 0 if report['completed'] == 2 and report['fatal'] is None else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, default=Path('.cache/ace16-index-split-acceptance-next-20261027/plan.json'))
    parser.add_argument('--verify-existing', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--artifact-sha256')
    parser.add_argument('--revision')
    parser.add_argument('--build-receipt', type=Path)
    parser.add_argument('--workdir', type=Path)
    args = parser.parse_args()
    assert capture.pin(args.plan)['sha256'] == PLAN_SHA256
    assert capture.pin(DIRECTORY / 'proof.py')['sha256'] == PROOF_SHA256
    plan = json.loads(capture.common.path(args.plan).read_text())
    for item in plan['immutable']:
        verify(item)
    if args.verify_existing is not None:
        if args.execute or args.output is None:
            parser.error('--verify-existing requires --output and excludes --execute')
        return verify_existing.run(args.verify_existing, args.output)
    if not args.execute:
        print('Two frozen selectors/pins verified; zero native/JVM calls.')
        return 0
    for field in ['artifact', 'artifact_sha256', 'revision', 'build_receipt', 'workdir']:
        if getattr(args, field) is None:
            parser.error('--execute requires --' + field.replace('_', '-'))
    return execute(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
