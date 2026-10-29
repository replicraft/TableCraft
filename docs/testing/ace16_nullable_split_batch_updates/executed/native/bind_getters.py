"""Bind fresh reads only after all twenty complete engineering images match."""
import json
from pathlib import Path

import bounded
import capture

HERE = Path(__file__).resolve().parent


def main():
    freeze, plan, _ = capture.validate_inputs()
    native_path = bounded.path(plan['native_workdir']) / 'report.json'
    report = json.loads(native_path.read_text())
    assert report['completed'] and report['phase'] == 'native' and not report['pin_errors']
    assert len(report['commands']) == len(report['whole_images']) == 20
    capture.all_images(plan)
    for command, case in zip(report['commands'], plan['cases']):
        assert command['argv'] == case['native']['argv'] and command['status'] == 0
        assert not command['capture_errors'] and command['stopped_for'] is None
        capture.native_streams(case, command)
    inputs = freeze['inputs'] + [bounded.pin(HERE / 'freeze.json'), bounded.pin(native_path)]
    inputs += report['artifacts']
    for case in plan['cases']:
        inputs.append(bounded.pin(case['output']))
    pins = {pin['path']: pin for pin in inputs}
    capture.require(pins.values())
    assert not bounded.path(plan['getter_workdir']).exists()
    with (HERE / 'getter-binding.json').open('x') as output:
        json.dump(dict(phase='fresh20-after-all20-exact-engineering-images',
                       native_report=bounded.pin(native_path), inputs=list(pins.values())), output, indent=2)
        output.write('\n')
    print(json.dumps(dict(binding=bounded.pin(HERE / 'getter-binding.json'), children=0)))


if __name__ == '__main__':
    main()
