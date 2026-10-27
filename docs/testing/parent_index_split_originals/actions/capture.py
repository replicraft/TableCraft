"""Reproduce fixed historical parent-overflow actions on fresh copies.

Maintained public wrapper, distinct from the archived first execution.
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
sys.path.insert(0, str(HERE))
from parent_index_split_originals import wire
from parent_index_split_originals.capture import Commands
from slot_preview_originals import common
import delete_reference_maps as maps
import model

PLAN = HERE / 'plan.json'
FREEZE = HERE / 'freeze.json'
REFERENCE = HERE / 'ParentSplitUpdateReference.java'
BOUNDED_CHANNEL = ROOT / 'docs/testing/parent_index_split_originals/BoundedFileChannel.java'
ORIGINAL_CLASSES = ROOT / '.cache/ace16-parent-split-reference-initial-20261027/classes'
JAR = ROOT / '.cache/tablecraft-fixtures/jackcess-5.0.4.jar'
COMPILED_REVISION = 'd8bdf6c3ec015afdb70ffa4668ac10974cf7dbb6'
ARTIFACT_SHA256 = 'd9facf656f8cb51e5e235223b5532ee64624695e0f1a165fee3b8c6a4ec614c3'
PROCESS_TIMEOUT_SECONDS = 1200


def pin(path):
    return common.pin(path)


def check_pin(expected):
    assert pin(expected['path']) == expected, expected['path']


def checked(command):
    assert command['status'] == 0 and not command['timed_out'], command
    assert not common.path(command['stderr']['path']).read_bytes(), command


def java(jar, classes, main, *arguments):
    return ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8', '-cp',
            os.pathsep.join([str(common.path(jar)), str(classes)]), main, *map(str, arguments)]


def describe(args, source, label, commands):
    original = pin(source)
    command = commands.run(java(args.jar, common.path(args.getter_classes), 'ParentSplitFixture', 'describe', source), label)
    checked(command)
    tables, map_text = wire.parse_description(common.path(command['stdout']['path']).read_text())
    raw = common.path(source).read_bytes()
    metadata = maps.parse(map_text)
    decoded = {name: maps.decode_map(raw, item, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
               for name, item in metadata['maps'].items()}
    root = tables['SparseInt32']['indexes']['PrimaryKey']['root']
    nodes, leaves, entries = wire.tree(raw, root, tables['SparseInt32']['owner'])
    assert sorted(map(int, nodes)) == decoded['IndexOwned']['file_members']
    assert [wire.key(entry) for entry in entries] == tables['SparseInt32']['indexes']['PrimaryKey']['order']
    assert pin(source) == original
    result = {'file': original, 'command': command, 'tables': tables, 'maps': decoded,
              'map_metadata': metadata, 'root': root, 'height': nodes[root]['height'],
              'leaf_partitions': [{'page': number, 'count': len(nodes[number]['entries'])}
                                  for number in leaves],
              'full_key_row_sha256': hashlib.sha256(b''.join(entries)).hexdigest()}
    return raw, result


def verify_inputs(args):
    manifest = None
    if args.freeze is not None:
        manifest = json.loads(common.path(args.freeze).read_text())
        for item in manifest['sources']:
            check_pin(item)
    plan = json.loads(common.path(args.plan).read_text())
    check_pin(plan['source_phase_report'])
    assert pin(args.jar)['sha256'] == common.JAR_SHA256
    for case in plan['cases']:
        check_pin(case['source'])
        assert (case['ordinal'], case['expected'], case['replacement']) == (781, 99424, 2147483647)
        assert case['table'] == 'SparseInt32' and case['column'] == 'Key'
    getter_classes = []
    for item in plan['unchanged_ancestors_and_tools']:
        check_pin(item)
        if item['path'].endswith('.class'):
            actual = pin(common.path(args.getter_classes) / Path(item['path']).name)
            assert (actual['bytes'], actual['sha256']) == (item['bytes'], item['sha256'])
            getter_classes.append(actual)
    build = json.loads(common.path(args.build_receipt).read_text())
    assert build['compiled_revision'] == COMPILED_REVISION and build['exit_code'] == 0
    artifact = pin(args.artifact)
    assert (artifact['bytes'], artifact['sha256']) == (build['frozen_example']['bytes'], build['frozen_example']['sha256'])
    assert build['frozen_example']['sha256'] == ARTIFACT_SHA256
    for source in build['source_pins']:
        data = subprocess.run(['git', 'show', COMPILED_REVISION + ':' + source['path']], cwd=ROOT,
                              check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        assert len(data) == source['bytes']
        assert hashlib.sha256(data).hexdigest() == source['sha256']
    return plan, manifest, {'receipt': pin(args.build_receipt), 'artifact': pin(args.artifact),
                            'compiled_revision': COMPILED_REVISION,
                            'historical_selected_git_objects': len(build['source_pins']),
                            'actual_getter_classes': getter_classes}


def execute(args, plan, manifest, build):
    directory = common.fresh_work(args.workdir)
    commands = Commands(directory)
    archive = directory / 'precall-sources'
    archive.mkdir()
    archived = []
    for position, source in enumerate(manifest['sources']):
        target = archive / (str(position) + '-' + Path(source['path']).name)
        target.write_bytes(common.path(source['path']).read_bytes())
        archived.append(pin(target))
    precall = directory / 'before-first-command.json'
    precall.write_text(json.dumps({'plan': pin(args.plan), 'freeze': pin(args.freeze),
                                  'source_archives': archived, 'build': build}, indent=2) + '\n')
    report = {'scope': 'Two current-service attempts and two public UPDATEs; actual root growth classified from saved results, not assumed',
              'precall': pin(precall), 'cases': [], 'completed_profiles': 0}
    output = directory / 'report.json'
    classes = directory / 'classes'
    classes.mkdir()
    compiled = commands.run(['javac', '-cp', str(common.path(args.jar)), '-d', str(classes),
                              str(BOUNDED_CHANNEL), str(REFERENCE)], 'compile')
    report['compile'] = compiled
    report['commands'] = commands.records
    output.write_text(json.dumps(report, indent=2) + '\n')
    checked(compiled)
    report['classes'] = [pin(path) for path in sorted(classes.glob('*.class'))]
    for case in plan['cases']:
        profile = case['profile']
        item = {'profile': profile, 'selector': case, 'complete': False}
        report['cases'].append(item)
        try:
            source = common.path(case['source']['path'])
            before_raw, before = describe(args, source, profile + '-before', commands)
            item['before'] = before
            assert before['height'] == 1 and before['root'] == case['root']
            candidate = directory / (profile + '-current-service-candidate.accdb')
            attempt = commands.run([
                str(common.path(args.artifact)), str(source), str(candidate), case['table'], case['column'],
                str(case['ordinal']), str(case['expected']), str(case['replacement']),
            ], profile + '-current-service-attempt')
            item['native_attempt'] = attempt
            item['native_actual_error'] = common.path(attempt['stderr']['path']).read_text()
            item['native_candidate'] = pin(candidate) if candidate.exists() else None
            item['native_atomic_refusal'] = (attempt['status'] != 0 and not attempt['timed_out']
                                             and not candidate.exists()
                                             and not common.path(attempt['stdout']['path']).read_bytes())
            check_pin(case['source'])
            reference = directory / (profile + '-public-reference.accdb')
            action = commands.run(java(args.jar, classes, 'ParentSplitUpdateReference', source, reference,
                                       case['source']['sha256'], case['page'], case['slot']),
                                  profile + '-public-update')
            item['public_action'] = action
            item['public_copy'] = pin(reference) if reference.exists() else None
            checked(action)
            after_raw, after = describe(args, reference, profile + '-after', commands)
            item['after'] = after
            item['comparison'] = model.compare(before_raw, after_raw,
                                                before['tables'], after['tables'],
                                                before['maps'], after['maps'], wire)
            check_pin(case['source'])
            item['complete'] = True
            report['completed_profiles'] += 1
        except Exception as error:
            item['failure'] = {'class': type(error).__name__, 'message': str(error),
                               'traceback': traceback.format_exc()}
        finally:
            check_pin(case['source'])
            report['commands'] = commands.records
            report['process_count'] = len(commands.records)
            output.write_text(json.dumps(report, indent=2) + '\n')
    for item in manifest['sources']:
        check_pin(item)
    for item in plan['unchanged_ancestors_and_tools']:
        check_pin(item)
    report['immutable_sources_and_inputs_unchanged'] = True
    report['atomic_native_refusals'] = sum(case.get('native_atomic_refusal', False) for case in report['cases'])
    report['observed_root_growths'] = sum(case.get('comparison', {}).get('actual_root_growth', False)
                                        for case in report['cases'])
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': pin(output), 'complete_profiles': report['completed_profiles'],
                      'atomic_native_refusals': report['atomic_native_refusals'],
                      'actual_root_growths': report['observed_root_growths']}))
    return 0 if report['completed_profiles'] == 2 and report['atomic_native_refusals'] == 2 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, default=PLAN)
    parser.add_argument('--freeze', type=Path)
    parser.add_argument('--freeze-current', type=Path)
    parser.add_argument('--jar', type=Path, default=JAR)
    parser.add_argument('--getter-classes', type=Path, default=ORIGINAL_CLASSES)
    parser.add_argument('--artifact', type=Path,
                        default=Path('.cache/tablecraft-bin/ace16-nonroot-split-20261027'))
    parser.add_argument('--build-receipt', type=Path,
                        default=Path('docs/testing/results/ace16-nonroot-split-example-build-2026-10-27.json'))
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    plan, manifest, build = verify_inputs(args)
    if args.freeze_current is not None:
        assert not args.execute
        target = common.path(args.freeze_current)
        target.relative_to(ROOT / '.cache')
        sources = [HERE / name for name in ('capture.py', 'model.py', 'ParentSplitUpdateReference.java', 'plan.json', 'framing.py', 'fresh_framing.py', 'verify_existing.py')]
        sources.extend([BOUNDED_CHANNEL, ROOT / 'docs/testing/parent_index_split_originals/capture.py', ROOT / 'docs/testing/parent_index_split_originals/wire.py', ROOT / 'docs/testing/slot_preview_originals/common.py', ROOT / 'docs/testing/delete_reference_maps.py'])
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('x') as output:
            json.dump({'sources': [pin(path) for path in sources],
                       'scope': 'Current public action wrapper sources, not historical execution'}, output, indent=2)
            output.write('\n')
        print(json.dumps(pin(target)))
        return 0
    if not args.execute:
        print('Fixed sources, current frozen consumer and153 historical Git objects verified; no JVM/native actions.')
        return 0
    if args.workdir is None or manifest is None:
        parser.error('--execute requires a fresh --workdir and current-source --freeze')
    return execute(args, plan, manifest, build)


if __name__ == '__main__':
    raise SystemExit(main())
