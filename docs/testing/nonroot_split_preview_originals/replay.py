"""Artifact-gated original non-root split candidates; no implicit actions."""
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
from nonroot_split_preview_originals import proof, verify_existing
from nonroot_index_split_originals import capture as evidence
from slot_preview_originals import common

PLAN_SHA256 = 'fc30b8f73ef5d26502b9d13c68d9bbf8591b571a7faf31315fe4914ce4eea87f'
PROOF_SHA256 = 'ca0d81e7fe99729ed87be03eb4e160f758754373ea6bc7def0ea11d40424648e'
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
JAR = Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar')


def verify(pin):
    assert common.pin(pin['path']) == pin


def gate(args):
    receipt = json.loads(common.path(args.build_receipt).read_text())
    assert receipt['exit_code'] == 0 and receipt['compiled_revision'] == args.revision
    artifact = common.pin(args.artifact)
    assert artifact == receipt['frozen_example'] and artifact['sha256'] == args.artifact_sha256
    consumer = False
    for pin in receipt['source_pins']:
        process = subprocess.run(['git', 'show', args.revision + ':' + pin['path']], cwd=ROOT,
                                 capture_output=True, check=True, timeout=30)
        assert len(process.stdout) == pin['bytes']
        assert hashlib.sha256(process.stdout).hexdigest() == pin['sha256']
        if pin['path'] == 'crates/engines/tablecraft-ace/examples/preview_ace16_split.rs':
            assert b'preview_ace16_split_int32_updates' in process.stdout
            consumer = True
    assert consumer
    return {'receipt': common.pin(args.build_receipt), 'artifact': artifact,
            'compiled_revision': args.revision, 'source_pins': receipt['source_pins']}


def semantic(actual, reference):
    assert actual['metadata'].keys() == reference['metadata'].keys()
    for name, table in actual['metadata'].items():
        expected = reference['metadata'][name]
        assert table == expected, (name, 'complete typed/index results')
        for root, tree in actual['geometry'][name]['trees'].items():
            assert tree['entry_hex'] == reference['geometry'][name]['trees'][root]['entry_hex']
    return True


def restore(state):
    result = copy.deepcopy(state)
    for command in ['getter', 'map_probe']:
        for stream in ['stdout', 'stderr']:
            verify(state[command][stream])
    result['metadata'] = evidence.trees.parse(common.path(state['getter']['stdout']['path']).read_text())
    return result


def execute(args, plan):
    build = gate(args)
    assert common.pin(JAR)['sha256'] == common.JAR_SHA256
    source_report = json.loads(common.path(plan['source']['path']).read_text())
    dependencies = [Path(__file__), DIRECTORY / 'proof.py', args.plan,
                    Path(evidence.__file__).resolve(), evidence.GETTER, evidence.REFERENCE,
                    Path('docs/testing/capture_delete_trees.py'), Path('docs/testing/delete_reference_maps.py'),
                    Path('docs/testing/index_split_originals/capture.py'), Path('docs/testing/slot_preview_originals/common.py'), JAR]
    immutable = [common.pin(path) for path in dependencies]
    immutable.extend([plan['source'], plan['handoff'], build['receipt'], build['artifact']])
    for case, observed in zip(plan['cases'], source_report['cases'], strict=True):
        immutable.extend([case['original'], case['public_reference']])
        for state in [observed['before'], observed['after']]:
            for command in ['getter', 'map_probe']:
                immutable.extend([state[command]['stdout'], state[command]['stderr']])
    for pin in immutable:
        verify(pin)
    directory = common.fresh_work(args.workdir)
    archives = []
    archive = directory / 'executed-sources'
    archive.mkdir()
    for path in [Path(__file__), DIRECTORY / 'proof.py', common.path(args.plan)]:
        destination = archive / path.name
        shutil.copyfile(path, destination)
        archives.append({'source': common.pin(path), 'archive': common.pin(destination)})
    requests = {}
    for case in plan['cases']:
        candidate = directory / (case['profile'] + '-candidate.accdb')
        argv = [build['artifact']['path'], case['original']['path'], str(candidate.relative_to(ROOT)),
                'Tree', 'ID', '27', '1000', '2147483647']
        path = directory / (case['profile'] + '-request.json')
        path.write_text(json.dumps({'argv': argv}, indent=2) + '\n')
        requests[case['profile']] = {'argv': argv, 'pin': common.pin(path)}
        immutable.append(common.pin(path))
    before = directory / 'before-first-call.json'
    before.write_text(json.dumps({'immutable': immutable, 'archives': archives, 'build': build}, indent=2) + '\n')
    commands = common.Capture(directory)
    report = {'scope': 'Two original non-root candidates; preserve old LVAL descriptors/pages independently of public rewrite',
              'build': build, 'before_first_call': common.pin(before), 'cases': [], 'completed': 0, 'fatal': None}
    output = directory / 'report.json'
    try:
        for case, observed in zip(plan['cases'], source_report['cases'], strict=True):
            candidate = directory / (case['profile'] + '-candidate.accdb')
            item = {'profile': case['profile'], 'selector': case, 'complete': False,
                    'request': requests[case['profile']]['pin'], 'output_path': str(candidate.relative_to(ROOT))}
            report['cases'].append(item)
            try:
                command = commands.run(requests[case['profile']]['argv'], case['profile'] + '-candidate')
                item['command'] = command
                if candidate.exists():
                    item['candidate'] = common.pin(candidate)
                assert command['status'] == 0 and not command['timed_out']
                assert common.path(command['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
                assert common.path(command['stderr']['path']).read_text() == NOTICE
                raw = candidate.read_bytes()
                original = common.path(case['original']['path']).read_bytes()
                item['whole_image_proof'] = proof.prove(case, original, raw)
                args.jar = JAR
                _, state = evidence.capture(args, candidate.relative_to(ROOT), case['profile'] + '-candidate', commands)
                item['after'] = state
                item['public_semantic_agreement'] = semantic(state, restore(observed['after']))
                old = restore(observed['before'])
                fixed = {'expected': 1000, 'replacement': 2147483647,
                         'record_reference_hex': case['original_reference_hex']}
                item['original_comparison'] = evidence.compare(fixed, (original, old), (raw, state))
                assert all(value['before'] == value['after'] for value in item['original_comparison']['observed_historical_counters'])
                item['complete'] = True
                report['completed'] += 1
            except Exception as error:
                item['error'] = {'class': type(error).__name__, 'message': str(error), 'traceback': traceback.format_exc()}
            finally:
                if candidate.exists():
                    item['candidate'] = common.pin(candidate)
                report['commands'] = commands.commands
                output.write_text(json.dumps(report, indent=2) + '\n')
        for pin in immutable:
            verify(pin)
        report['immutable_inputs_unchanged'] = True
    except Exception as error:
        report['fatal'] = {'class': type(error).__name__, 'message': str(error), 'traceback': traceback.format_exc()}
    report['commands'] = commands.commands
    report['command_count'] = len(commands.commands)
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': common.pin(output), 'completed': report['completed'], 'commands': report['command_count'], 'fatal': report['fatal']}))
    return 0 if report['completed'] == 2 and report['fatal'] is None else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, default=Path('.cache/ace16-nonroot-split-acceptance-next-20261027/plan.json'))
    parser.add_argument('--verify-existing', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--execute', action='store_true')
    for name in ['artifact', 'build-receipt', 'workdir']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--artifact-sha256')
    parser.add_argument('--revision')
    args = parser.parse_args()
    assert common.pin(args.plan)['sha256'] == PLAN_SHA256
    assert common.pin(DIRECTORY / 'proof.py')['sha256'] == PROOF_SHA256
    plan = json.loads(common.path(args.plan).read_text())
    for pin in [plan['source'], plan['handoff'], plan['proof']]:
        verify(pin)
    for case in plan['cases']:
        verify(case['original'])
        verify(case['public_reference'])
    if args.verify_existing is not None:
        if args.execute or args.output is None:
            parser.error('--verify-existing requires --output and excludes --execute')
        return verify_existing.run(args.verify_existing, args.output)
    if not args.execute:
        print('Frozen non-root two-case plan verified; zero native/JVM calls.')
        return 0
    for name in ['artifact', 'build_receipt', 'workdir', 'artifact_sha256', 'revision']:
        if getattr(args, name) is None:
            parser.error('--execute requires --' + name.replace('_', '-'))
    return execute(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
