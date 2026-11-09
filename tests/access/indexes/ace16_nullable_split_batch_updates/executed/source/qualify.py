"""All eight saved target gates before any reference or image construction."""
import json
from pathlib import Path

import bounded
import source
import source_warnings

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = '.cache/ace16-nullable-split-batch-source-first-20261029'


def main():
    plan = json.loads((HERE / 'source-plan.json').read_text())
    freeze_pin = bounded.pin(HERE / 'source-freeze.json')
    freeze = json.loads((HERE / 'source-freeze.json').read_text())
    assert freeze['phase'] == plan['phase']
    report_path = ROOT / WORK / 'report.json'
    report_pin = bounded.pin(report_path)
    report = json.loads(report_path.read_text())
    assert report['completed'] and report['source_qualification'] is False
    assert len(report['commands']) == 9 and len(report['images']) == 4
    assert report['inputs'] == freeze['inputs'] + [freeze_pin]
    assert not report['pin_errors'] and report['public_reference_commands'] == 0
    for actual, expected in zip(report['commands'], plan['commands'], strict=True):
        assert actual['argv'] == expected['argv'] and actual['label'] == expected['id']
        assert actual['status'] == 0 and actual['stopped_for'] is None
        assert not actual['capture_errors'] and actual['timeout_seconds'] == 30
        assert actual['stream_limit_bytes'] == 16777216
        stderr = bounded.path(actual['stderr']['path']).read_bytes()
        if expected['id'].endswith('-create'):
            case = next(item for item in plan['cases']
                        if expected['id'] == item['id'] + '-create')
            source_warnings.require_create(stderr, case['source'])
        else:
            assert stderr == b''
    pins = report['inputs'] + report['classes'] + report['artifacts'] + report['images'] + [report_pin]
    pins += [command[stream] for command in report['commands'] for stream in ('stdout', 'stderr')]
    for item in pins:
        bounded.require_pin(item)
    facts = []
    for case in plan['cases']:
        getter = WORK + '/' + case['id'] + '-inspect.stdout'
        assert bounded.pin(case['source']) in report['images']
        state = source.inspect(ROOT / case['source'], ROOT / getter,
                               case['profile'], case['direction'])
        facts.append(dict(id=case['id'], profile=case['profile'],
                          direction=case['direction'], source=bounded.pin(case['source']),
                          getter=bounded.pin(getter), facts=state))
    assert sum(len(item['facts']['tables']) - 1 for item in facts) == 8
    for item in pins:
        bounded.require_pin(item)
    result = dict(source_qualification=True, source_gates=8, sentinel_gates=4,
                  actual_child_commands=0, constructor_images=0,
                  report=report_pin, pins=pins, facts=facts)
    output = HERE / 'source-gates-first.json'
    with output.open('x') as handle:
        handle.write(json.dumps(result, indent=2) + '\n')
    print(json.dumps(dict(qualified=True, output=bounded.pin(output)), indent=2))


if __name__ == '__main__':
    main()
