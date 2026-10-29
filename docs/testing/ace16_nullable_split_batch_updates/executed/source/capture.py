"""Exact SOURCE 9 capture; qualification is a separate saved-only operation."""
import argparse
import json
from pathlib import Path
import shutil

import bounded
import source_warnings

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = '.cache/ace16-nullable-split-batch-source-first-20261029'
REFERENCE = '.cache/ace16-nullable-split-batch-reference-first-20261029'


def validate(freeze_path):
    freeze = json.loads(freeze_path.read_text())
    assert freeze['phase'] == 'nullable-split-batch-source9-capture-only'
    for item in freeze['inputs']:
        bounded.require_pin(item)
    plan = json.loads((HERE / 'source-plan.json').read_text())
    assert plan['phase'] == freeze['phase']
    assert plan['source_qualification'] is False
    assert plan['source_children'] == 9 and plan['future_reference_children'] == 40
    assert plan['limits'] == dict(file_bytes=8388608, stdout_bytes=16777216,
                                  stderr_bytes=16777216, heap_bytes=536870912, child_seconds=30)
    cases = [dict(id=layout + '-' + direction, profile=layout, direction=direction,
                  source=WORK + '/' + layout + '-' + direction + '-source.accdb')
             for layout in ('V2016', 'V2019') for direction in ('ASC', 'DESC')]
    assert plan['cases'] == cases
    jar = '.cache/tablecraft-fixtures/jackcess-5.0.4.jar'
    expected = [dict(id='compile', argv=[
        'javac', '-J-Xmx512m', '-encoding', 'UTF-8', '-cp', jar,
        '-d', WORK + '/classes',
        'docs/testing/parent_index_split_originals/BoundedFileChannel.java',
        str(HERE.relative_to(ROOT) / 'NullableSplitBatchFixture.java')])]
    common = ['java', '-Xmx512m', '-Duser.timezone=UTC', '-Duser.language=en',
              '-Duser.country=US', '-Djava.util.logging.SimpleFormatter.format=' + source_warnings.FORMAT,
              '-cp', jar + ':' + WORK + '/classes', 'NullableSplitBatchFixture']
    for operation in ('create', 'inspect'):
        for case in cases:
            argv = common + [operation, case['source'], case['profile']]
            if operation == 'create':
                argv.append(case['direction'])
            expected.append(dict(id=case['id'] + '-' + operation, argv=argv))
    assert plan['commands'] == expected and len(expected) == 9
    assert plan['create_warning_tables'] == list(source_warnings.TABLES)
    assert plan['targets'] == ['Items', 'UniqueItems']
    assert not (ROOT / WORK).exists(), 'Preserve first attempt in its fresh namespace'
    assert not (ROOT / REFERENCE).exists(), 'SOURCE 9 authorizes no reference transition'
    return freeze, plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze, plan = validate(args.freeze)
    if not args.execute:
        print(json.dumps(dict(static_validated=True, inputs=len(freeze['inputs']),
                              source_commands=9, source_qualification=False,
                              actual_child_commands=0), indent=2))
        return
    inputs = freeze['inputs'] + [bounded.pin(args.freeze)]
    work = ROOT / WORK
    work.mkdir()
    (work / 'classes').mkdir()
    capture = bounded.Capture(work)
    images, classes, streams = [], [], []
    result = dict(completed=False, source_qualification=False, inputs=inputs,
                  commands=capture.records, images=images, classes=classes,
                  pin_errors=[], artifacts=[], public_reference_commands=0)
    try:
        archive = work / 'precall-inputs'
        archive.mkdir()
        for position, item in enumerate(inputs):
            bounded.require_pin(item)
            shutil.copyfile(ROOT / item['path'], archive / f'{position:03d}-{Path(item["path"]).name}')
        for command in plan['commands']:
            for item in inputs + images + classes + streams:
                bounded.require_pin(item)
            result['active_command'] = command['id']
            stdout = capture.run(command['argv'], command['id'], 30)
            streams.extend(capture.records[-1][name] for name in ('stdout', 'stderr'))
            stderr = bounded.path(capture.records[-1]['stderr']['path']).read_bytes()
            if command['id'].endswith('-create'):
                selected = next(item for item in plan['cases']
                                if command['id'] == item['id'] + '-create')
                source_warnings.require_create(stderr, selected['source'])
            else:
                assert stderr == b''
            if command['id'] == 'compile':
                assert stdout.read_bytes() == b''
                files = sorted((work / 'classes').rglob('*.class'))
                assert 2 <= len(files) <= 8
                assert (work / 'classes/NullableSplitBatchFixture.class').is_file()
                assert (work / 'classes/BoundedFileChannel.class').is_file()
                classes.extend(bounded.pin(path) for path in files)
            elif command['id'].endswith('-create'):
                case = next(item for item in plan['cases']
                            if command['id'] == item['id'] + '-create')
                expected_stdout = ('CREATED\t' + case['profile'] + '\t'
                                   + case['direction'] + '\n').encode()
                assert stdout.read_bytes() == expected_stdout
                image = bounded.pin(case['source'])
                assert 0 < image['bytes'] <= 8388608 and image['bytes'] % 4096 == 0
                images.append(image)
            else:
                assert stdout.stat().st_size > 0
        result['completed'] = len(capture.records) == 9 and len(images) == 4
    except BaseException as error:
        result['failure'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        for item in inputs + images + classes + streams:
            try:
                bounded.require_pin(item)
            except BaseException as error:
                result['pin_errors'].append(dict(path=item['path'], error=str(error)))
        for path in sorted(work.rglob('*')):
            if path.is_file() and path != work / 'report.json':
                result['artifacts'].append(bounded.pin(path))
        result['completed'] = result['completed'] and not result['pin_errors']
        with (work / 'report.json').open('x') as handle:
            handle.write(json.dumps(result, indent=2) + '\n')
    assert result['completed'], 'Retain first failure; no retry or source repair'


if __name__ == '__main__':
    main()
