"""Exactly twenty fresh public actions and twenty separately launched inert reads."""
import argparse
import json
from pathlib import Path
import shutil

import barriers
import intent
import reference
import reference_bounded as bounded

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = '.cache/ace16-nullable-split-batch-reference-first-20261029'
COMPILE = '.cache/ace16-nullable-split-batch-action-compile-first-20261029'


def validate(freeze_path):
    freeze = json.loads(freeze_path.read_text())
    assert freeze['phase'] == 'nullable-split-batch-public-reference40'
    for item in freeze['inputs']:
        bounded.require_pin(item)
    plan = json.loads((HERE / 'reference-plan.json').read_text())
    gates, expected = barriers.require_plan(plan)
    compile_path = ROOT / COMPILE / 'report.json'
    compiled = json.loads(compile_path.read_text())
    assert compiled['completed'] is True and not compiled['pin_errors']
    assert compiled['public_reference_commands'] == compiled['native_commands'] == 0
    assert len(compiled['commands']) == len(compiled['classes']) == 1
    actual = compiled['commands'][0]
    compile_plan = json.loads((HERE / 'compile-plan.json').read_text())
    assert actual['argv'] == compile_plan['commands'][0]['argv']
    assert actual['label'] == 'compile-action' and actual['status'] == 0
    assert actual['stopped_for'] is None and not actual['capture_errors']
    assert actual['timeout_seconds'] == 30 and actual['stream_limit_bytes'] == 16777216
    for stream in ('stdout', 'stderr'):
        assert bounded.path(actual[stream]['path']).read_bytes() == b''
    assert compiled['classes'][0]['path'] == COMPILE + '/classes/NullableSplitBatchAction.class'
    required = (gates['pins'] + expected['inputs'] + expected['outputs']
                + compiled['inputs'] + compiled['artifacts'] + compiled['classes']
                + [bounded.pin(compile_path),
                   bounded.pin(barriers.SOURCE / 'source-gates-first.json'),
                   bounded.pin(barriers.OUTPUT / 'freeze.json')])
    for item in required:
        bounded.require_pin(item)
        assert item in freeze['inputs'], ('Missing immutable barrier input', item['path'])
    assert not (ROOT / WORK).exists(), 'Retain the first reference attempt; no retries'
    return freeze, plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze', required=True, type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze, plan = validate(args.freeze)
    if not args.execute:
        print(json.dumps(dict(static_validated=True, inputs=len(freeze['inputs']),
                              source_gates=8, expected_images=20, compile_commands=0,
                              reference_commands=40, actual_child_commands=0), indent=2))
        return
    inputs = freeze['inputs'] + [bounded.pin(args.freeze)]
    work = ROOT / WORK
    work.mkdir()
    capture = bounded.Capture(work)
    images, streams, proofs = [], [], []
    result = dict(completed=False, inputs=inputs, commands=capture.records, images=images,
                  proofs=proofs, pin_errors=[], artifacts=[], source_gates=8,
                  expected_images_before_first_action=20, constructor_images=0,
                  native_commands=0, compile_commands=0)
    try:
        archive = work / 'precall-inputs'
        archive.mkdir()
        for position, item in enumerate(inputs):
            bounded.require_pin(item)
            shutil.copyfile(ROOT / item['path'], archive / f'{position:03d}-{Path(item["path"]).name}')
        for position, command in enumerate(plan['commands']):
            for item in inputs + images + streams:
                bounded.require_pin(item)
            case = plan['cases'][position // 2]
            result['active_command'] = command['id']
            stdout = capture.run(command['argv'], command['id'], 30)
            record = capture.records[-1]
            streams.extend(record[name] for name in ('stdout', 'stderr'))
            if position % 2 == 0:
                original = reference.state(bounded.path(case['source']).read_bytes(),
                                           bounded.path(case['before_getter']).read_text())
                observed = intent.derive(original['tables'][case['table']], case)
                intent.check_streams(stdout.read_bytes(), bounded.path(record['stderr']['path']).read_bytes(),
                                     case, observed)
                image = bounded.pin(case['output'])
                assert bounded.pin(case['source'])['bytes'] <= image['bytes'] <= 8388608
                assert image['bytes'] % 4096 == 0
                images.append(image)
            else:
                assert bounded.path(record['stderr']['path']).read_bytes() == b''
                action = capture.records[-2]
                facts = reference.compare(case, stdout, bounded.path(action['stdout']['path']),
                                          bounded.path(action['stderr']['path']))
                proofs.append(dict(id=case['id'], facts=facts, output=bounded.pin(case['output']),
                                   getter=record['stdout']))
            for item in inputs + images + streams:
                bounded.require_pin(item)
        result['completed'] = len(capture.records) == 40 and len(images) == len(proofs) == 20
    except BaseException as error:
        result['failure'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        for item in inputs + images + streams:
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
    assert result['completed'], 'Preserve first failure; no repair or repeated action'


if __name__ == '__main__':
    main()
