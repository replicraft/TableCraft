"""Separate finite javac1 phase, after all twenty independent images are frozen."""
import argparse
import json
from pathlib import Path
import shutil

import barriers
import compile_bounded as bounded

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = '.cache/ace16-nullable-split-batch-action-compile-first-20261029'


def validate(freeze_path):
    freeze = json.loads(freeze_path.read_text())
    assert freeze['phase'] == 'nullable-split-batch-action-compile1'
    for item in freeze['inputs']:
        bounded.require_pin(item)
    plan = json.loads((HERE / 'compile-plan.json').read_text())
    reference_plan = json.loads((HERE / 'reference-plan.json').read_text())
    gates, expected_images = barriers.require_plan(reference_plan)
    required = (gates['pins'] + expected_images['inputs'] + expected_images['outputs']
                + [bounded.pin(barriers.SOURCE / 'source-gates-first.json'),
                   bounded.pin(barriers.OUTPUT / 'freeze.json')])
    for item in required:
        assert item in freeze['inputs'], ('Missing immutable barrier input', item['path'])
    assert plan['phase'] == freeze['phase'] and plan['work'] == WORK
    for field in ('expected_input_freeze', 'expected_output_freeze', 'expected_report',
                  'source_gate_receipt', 'limits'):
        assert plan[field] == reference_plan[field]
    source_classes = '.cache/ace16-nullable-split-batch-source-first-20261029/classes'
    expected = [dict(id='compile-action', argv=[
        'javac', '-J-Xmx512m', '-encoding', 'UTF-8', '-cp',
        '.cache/tablecraft-fixtures/jackcess-5.0.4.jar:' + source_classes,
        '-d', WORK + '/classes', str(HERE.relative_to(ROOT) / 'NullableSplitBatchAction.java')])]
    assert plan['commands'] == expected
    assert not (ROOT / WORK).exists(), 'Preserve first compile; no namespace reuse'
    assert not (ROOT / '.cache/ace16-nullable-split-batch-reference-first-20261029').exists()
    return freeze, plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze', required=True, type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze, plan = validate(args.freeze)
    if not args.execute:
        print(json.dumps(dict(static_validated=True, inputs=len(freeze['inputs']),
                              source_gates=8, expected_images=20,
                              compile_commands=1, actual_child_commands=0), indent=2))
        return
    inputs = freeze['inputs'] + [bounded.pin(args.freeze)]
    work = ROOT / WORK
    work.mkdir()
    (work / 'classes').mkdir()
    capture = bounded.Capture(work)
    classes, streams = [], []
    result = dict(completed=False, inputs=inputs, commands=capture.records, classes=classes,
                  artifacts=[], pin_errors=[], source_gates=8, expected_images=20,
                  public_reference_commands=0, native_commands=0)
    try:
        archive = work / 'precall-inputs'
        archive.mkdir()
        for position, item in enumerate(inputs):
            bounded.require_pin(item)
            shutil.copyfile(ROOT / item['path'], archive / f'{position:03d}-{Path(item["path"]).name}')
        command = plan['commands'][0]
        for item in inputs:
            bounded.require_pin(item)
        capture.run(command['argv'], command['id'], 30)
        record = capture.records[-1]
        streams.extend(record[name] for name in ('stdout', 'stderr'))
        assert all(bounded.path(item['path']).read_bytes() == b'' for item in streams)
        files = sorted((work / 'classes').rglob('*.class'))
        assert files == [work / 'classes/NullableSplitBatchAction.class']
        classes.extend(bounded.pin(path) for path in files)
        result['completed'] = len(capture.records) == 1
    except BaseException as error:
        result['failure'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        for item in inputs + classes + streams:
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
    assert result['completed'], 'Retain first compile failure; no retry'


if __name__ == '__main__':
    main()
