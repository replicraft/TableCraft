"""Twenty exact engineering images must precede twenty separately bound readers."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys

import bounded

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCE = ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029'
REFERENCE = ROOT / '.cache/ace16-nullable-split-batch-reference-next-20261029'
NOTICE = b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\n'


def require(items):
    for item in items:
        bounded.require_pin(item)


def proof_module():
    sys.path.insert(0, str(REFERENCE))
    spec = importlib.util.spec_from_file_location('native_split_batch_proof', HERE / 'proof.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def equal(case):
    bounded.require_pin(case['expected'])
    assert bounded.path(case['output']).read_bytes() == bounded.path(case['expected']['path']).read_bytes(), case['id']


def all_images(plan):
    for case in plan['cases']:
        equal(case)


def classes(plan):
    directory = bounded.path(plan['classes_directory'])
    actual = sorted(path.relative_to(ROOT).as_posix() for path in directory.rglob('*.class'))
    assert actual == [item['path'] for item in plan['classes']]
    require(plan['classes'])


def native_streams(case, report):
    stdout = bounded.path(report['stdout']['path']).read_bytes()
    stderr = bounded.path(report['stderr']['path']).read_bytes()
    assert stdout == f"candidate_changed_rows={case['changed_rows']}\n".encode()
    assert stderr == NOTICE


def validate_inputs():
    freeze = json.loads((HERE / 'freeze.json').read_text())
    assert freeze['phase'] == 'nullable-split-batch-native20-before-fresh20'
    require(freeze['inputs'])
    plan = json.loads((HERE / 'plan.json').read_text())
    assert plan['phase'] == freeze['phase'] and plan['public_after_authority'] is False
    assert len(plan['cases']) == 20
    classes(plan)
    receipt = json.loads(bounded.path(plan['receipt']['path']).read_text())
    assert receipt['commit'] == '50d028cc19452c54046b1e1d23baa77ac80f491b'
    assert receipt['status'] == 0 and receipt['artifacts']['batch'] == plan['artifact']
    assert receipt['fresh_target_was_absent'] and receipt['owning_engine_compilation_observed']
    assert receipt['source_inventory_unchanged_after_build'] and receipt['exported_file_count'] == 4791
    proof = proof_module()
    gates = json.loads((SOURCE / 'source-gates-first.json').read_text())
    assert gates['source_qualification'] and gates['source_gates'] == 8 and gates['sentinel_gates'] == 4
    for source in gates['facts']:
        actual = proof.source.inspect(bounded.path(source['source']['path']),
                                      bounded.path(source['getter']['path']),
                                      source['profile'], source['direction'])
        assert json.loads(json.dumps(actual)) == source['facts']
    known = {item['path'] for item in freeze['inputs']}
    for module in list(sys.modules.values()):
        filename = getattr(module, '__file__', None)
        if filename:
            path = Path(filename).resolve()
            if path.is_relative_to(ROOT) and path.suffix == '.py':
                assert path.relative_to(ROOT).as_posix() in known, str(path)
    for case in plan['cases']:
        bounded.require_pin(case['source'])
        bounded.require_pin(case['before_getter'])
        argv = [plan['artifact']['path'], case['source']['path'], case['output'], '--index-growth']
        for assignment in case['assignments']:
            expected = 'null' if assignment['expected'] is None else str(assignment['expected'])
            value = 'null' if assignment['value'] is None else str(assignment['value'])
            argv.extend(['--set', assignment['table'], assignment['column'], str(assignment['ordinal']),
                         'int32', expected, value])
        assert argv == case['native']['argv']
        assert case['getter']['argv'][-3:] == ['inspect', case['output'], case['request']['profile']]
    return freeze, plan, proof


def validate_binding(plan, binding_path):
    binding = json.loads(binding_path.read_text())
    require(binding['inputs'])
    assert binding['phase'] == 'fresh20-after-all20-exact-engineering-images'
    report = json.loads(bounded.path(binding['native_report']['path']).read_text())
    assert report['completed'] and report['phase'] == 'native' and not report['pin_errors']
    assert len(report['commands']) == len(report['whole_images']) == 20
    for command, case, image in zip(report['commands'], plan['cases'], report['whole_images']):
        assert command['argv'] == case['native']['argv'] and command['status'] == 0
        assert not command['capture_errors'] and command['stopped_for'] is None
        assert image['id'] == case['id'] and image['image'] == bounded.pin(case['output'])
        native_streams(case, command)
    all_images(plan)
    return binding


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--phase', choices=('native', 'getters'), required=True)
    parser.add_argument('--binding', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze, plan, proof = validate_inputs()
    inputs = freeze['inputs'] + [bounded.pin(HERE / 'freeze.json')]
    if args.phase == 'native':
        assert args.binding is None
        assert not bounded.path(plan['native_workdir']).exists()
        assert not bounded.path(plan['getter_workdir']).exists()
        assert all(not bounded.path(case['output']).exists() for case in plan['cases'])
        work = bounded.path(plan['native_workdir'])
    else:
        assert args.binding is not None
        binding = validate_binding(plan, args.binding)
        inputs += binding['inputs'] + [bounded.pin(args.binding)]
        work = bounded.path(plan['getter_workdir'])
        assert not work.exists()
    if not args.execute:
        print(json.dumps(dict(static=True, phase=args.phase, inputs=len(inputs),
                              planned_commands=20, source_gates=8, sentinel_gates=4, children=0)))
        return
    work.mkdir()
    runner = bounded.Capture(work)
    retained = []
    report = dict(completed=False, phase=args.phase, inputs=inputs, commands=runner.records,
                  artifacts=[], whole_images=[], proofs=[], pin_errors=[])
    try:
        for case in plan['cases']:
            require(inputs + retained)
            classes(plan)
            if args.phase == 'getters':
                all_images(plan)
            command = case['native' if args.phase == 'native' else 'getter']
            report['active_command'] = command['label']
            stdout = runner.run(command['argv'], command['label'])
            actual = runner.records[-1]
            retained.extend([actual['stdout'], actual['stderr']])
            if args.phase == 'native':
                native_streams(case, actual)
                equal(case)
                image = bounded.pin(case['output'])
                retained.append(image)
                report['whole_images'].append(dict(id=case['id'], image=image))
            else:
                assert bounded.path(actual['stderr']['path']).read_bytes() == b''
                native = bounded.path(plan['native_workdir'])
                facts = proof.compare(case['proof_case'], stdout,
                                      native / (case['id'] + '-native.stdout'),
                                      native / (case['id'] + '-native.stderr'))
                report['proofs'].append(dict(id=case['id'], facts=facts))
                all_images(plan)
            require(inputs + retained)
            classes(plan)
        report['completed'] = len(runner.records) == 20
    except BaseException as error:
        report['failure'] = type(error).__name__ + ': ' + str(error)
        raise
    finally:
        for item in inputs + retained:
            try:
                bounded.require_pin(item)
            except BaseException as error:
                report['pin_errors'].append(dict(path=item['path'], error=str(error)))
        for path in sorted(work.iterdir()):
            if path.is_file():
                report['artifacts'].append(bounded.pin(path))
        report['completed'] = report['completed'] and not report['pin_errors']
        with (work / 'report.json').open('x') as output:
            json.dump(report, output, indent=2)
            output.write('\n')
    assert report['completed'], 'Preserve first unexpected result; do not retry'


if __name__ == '__main__':
    main()
