(
    'Separate sixteen-candidate original batch supplement with explicit staging '
    'oracles.'
)
import argparse
import csv
import io
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from slot_preview_originals import batch, common, model
from slot_preview_originals.replay import parse, validate_build

ROOT = common.ROOT


def requests_bytes(scenario, reverse):
    stream = io.StringIO(newline='')
    writer = csv.writer(stream, delimiter='\t', lineterminator='\n')
    requests = list(reversed(scenario['requests'])) if reverse else scenario['requests']
    for request in requests:
        fields = [request['ordinal']]
        for name in ['expected', 'replacement']:
            kind, data = model.value(request[name])
            text = data.decode('utf-16-le') if kind == 'text' else data.hex()
            fields.extend([kind, text])
        writer.writerow(fields)
    return stream.getvalue().encode('utf-8')


def getter(capture, java, target, stem):
    before = common.pin(target)
    command = capture.run(java + ['oracle', str(target)], stem + '-getter')
    assert command['status'] == 0 and not common.path(command['stderr']['path']).read_bytes()
    assert common.pin(target) == before
    return parse(common.path(command['stdout']['path'])), command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--plan-sha256', required=True)
    parser.add_argument('--preparation', type=Path, required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--verify-existing', type=Path,
                        help='Verify captured final images/stages without reissuing operations')
    args = parser.parse_args()
    for name in ['plan', 'preparation', 'build_receipt', 'example', 'jar', 'classes', 'workdir', 'report']:
        setattr(args, name, common.path(getattr(args, name)))
    assert not args.workdir.exists() and not args.report.exists()
    args.workdir.relative_to(ROOT / '.cache')
    plan = json.loads(args.plan.read_text())
    assert common.pin(args.plan)['sha256'] == args.plan_sha256
    build = json.loads(args.build_receipt.read_text())
    validate_build(args.example, plan['frozen_consumer']['revision'], build)
    assert common.pin(args.example)['sha256'] == plan['frozen_consumer']['sha256']
    original_facts = {item['profile']: item for item in json.loads((ROOT / plan['source_facts']['path']).read_text())}
    prep = json.loads(args.preparation.read_text())
    protected = [ROOT / item['path'] for item in prep['inputs'] + prep['tools']]
    protected += [args.plan, Path(__file__), Path(batch.__file__), args.example, args.build_receipt,
                  ROOT / plan['source_facts']['path']]
    protected += [Path(batch.__file__), Path(common.__file__), Path(model.__file__)]
    for item in prep['inputs'] + prep['tools']:
        assert common.pin(ROOT / item['path']) == item
    report = {'scope': (
        'Separate final-image batch supplement; explicit public staging is not direct'
        ' per-row unique-intermediate or native SQL parity.'
    ),
              'plan': common.pin(args.plan), 'compiled_revision': build['compiled_revision'],
              'pins_before': [common.pin(path) for path in protected], 'scenarios': []}
    args.workdir.mkdir()
    capture = common.Capture(args.workdir)
    java = common.java(args.jar, args.classes)
    assert common.pin(args.jar) in prep['tools']
    assert common.pin(args.classes / 'SlotUpdateFixture.class') in prep['tools']
    if args.verify_existing:
        history = json.loads(common.path(args.verify_existing).read_text())
        assert history['compiled_revision'] == build['compiled_revision']
        assert history['plan'] == common.pin(args.plan)
        for item in history['scenarios']:
            scenario = item['scenario']
            original = original_facts[scenario['profile']]
            source = common.path(original['input']['path'])
            if 'direct_refusal' in item:
                assert item['direct_refusal']['command']['status'] not in [0, None]
                copy = common.path(item['direct_refusal']['copy']['path'])
                assert common.pin(copy) == item['direct_refusal']['copy']
                assert copy.read_bytes() == source.read_bytes()
            reference_item = item['references'][-1]
            reference_file = common.path(reference_item['copy']['path'])
            assert common.pin(reference_file) == reference_item['copy']
            getter_file = common.path(item['reference_final_getter']['stdout']['path'])
            assert common.pin(getter_file) == item['reference_final_getter']['stdout']
            reference = parse(getter_file)
            outputs = []
            for candidate in item['candidates']:
                target = common.path(candidate['output']['path'])
                assert common.pin(target) == candidate['output']
                request = common.path(candidate['request']['path'])
                assert common.pin(request) == candidate['request']
                assert request.read_bytes() == requests_bytes(scenario, candidate['reversed'])
                getter_file = common.path(candidate['getter']['stdout']['path'])
                assert common.pin(getter_file) == candidate['getter']['stdout']
                actual = parse(getter_file)
                result = batch.verify(source.read_bytes(), target.read_bytes(), original, actual,
                                      scenario, reference, reference_file.read_bytes())
                assert result == candidate['proof']
                outputs.append(target.read_bytes())
            assert len(outputs) == 2 and outputs[0] == outputs[1]
        report['scope'] = (
            'Current public-wrapper read-only batch verification; no native/public '
            'staging calls reissued.'
        )
        report['historical_report'] = common.pin(common.path(args.verify_existing))
        report['verified_scenarios'] = len(history['scenarios'])
        report['pins_after'] = [common.pin(path) for path in protected]
        assert report['pins_after'] == report['pins_before']
        args.report.write_text(json.dumps(report, indent=2) + '\n')
        print('Verified eight scenarios / sixteen existing candidates without reissuing actions')
        return 0
    for number, scenario in enumerate(plan['scenarios']):
        original = original_facts[scenario['profile']]
        source = ROOT / original['input']['path']
        assert common.pin(source) == original['input']
        profile = common.safe_label(scenario['profile'])
        label = common.safe_label(scenario['label'])
        stem = f'{number:02}-{profile}-{label}'
        item = {'scenario': scenario, 'references': [], 'candidates': []}
        report['scenarios'].append(item)
        try:
            direct = scenario['public_direct_unique_refusal']
            if direct:
                target = args.workdir / (stem + '-direct-refusal.accdb')
                command = capture.run(java + ['update', str(source), str(target), scenario['table'],
                                              str(direct['ID']), scenario['column'], direct['value']],
                                      stem + '-direct-refusal')
                assert command['status'] not in [0, None]
                assert target.read_bytes() == source.read_bytes()
                item['direct_refusal'] = {'command': command, 'copy': common.pin(target), 'copy_identical': True}
            previous = source
            for step, action in enumerate(scenario['reference_actions']):
                target = args.workdir / f'{stem}-reference-{step}.accdb'
                command = capture.run(java + ['update', str(previous), str(target), scenario['table'],
                                              str(action['ID']), scenario['column'], action['value']],
                                      stem + f'-reference-{step}')
                assert command['status'] == 0
                item['references'].append({'command': command, 'copy': common.pin(target)})
                previous = target
            reference, reference_getter = getter(capture, java, previous, stem + '-reference-final')
            batch.expected_rows(original['tables'], reference, scenario)
            item['reference_final_getter'] = reference_getter
            output_bytes = []
            for reverse in [False, True]:
                suffix = '-reversed' if reverse else '-physical'
                request = args.workdir / (stem + suffix + '.tsv')
                request.write_bytes(requests_bytes(scenario, reverse))
                target = args.workdir / (stem + suffix + '.accdb')
                candidate = {'reversed': reverse, 'request': common.pin(request)}
                item['candidates'].append(candidate)
                try:
                    command = capture.run([str(args.example), str(source), str(target), scenario['table'],
                                           scenario['column'], str(request)], stem + suffix)
                    candidate['command'] = command
                    assert command['status'] == 0 and target.exists(), 'Expected batch candidate refused'
                    expected_stdout = f"candidate_changed_rows={scenario['expected_changed_rows']}\n"
                    assert common.path(command['stdout']['path']).read_text() == expected_stdout
                    actual, actual_getter = getter(capture, java, target, stem + suffix)
                    candidate['getter'] = actual_getter
                    candidate['proof'] = batch.verify(source.read_bytes(), target.read_bytes(), original,
                                                        actual, scenario, reference, previous.read_bytes())
                    output_bytes.append(target.read_bytes())
                    candidate['agrees'] = True
                except Exception as error:
                    candidate['agrees'] = False
                    candidate['diagnostic'] = type(error).__name__ + ': ' + str(error)
                finally:
                    if target.exists():
                        candidate['output'] = common.pin(target)
            assert len(output_bytes) == 2 and output_bytes[0] == output_bytes[1]
            item['request_order_outputs_identical'] = True
            item['agrees'] = all(candidate['agrees'] for candidate in item['candidates'])
        except Exception as error:
            item['agrees'] = False
            item['diagnostic'] = type(error).__name__ + ': ' + str(error)
        finally:
            assert common.pin(source) == original['input']
            args.report.write_text(json.dumps(report, indent=2) + '\n')
        print(stem, 'agrees', item['agrees'], item.get('diagnostic', ''), flush=True)
    report['pins_after'] = [common.pin(path) for path in protected]
    report['all_protected_inputs_unchanged'] = report['pins_after'] == report['pins_before']
    report['scenario_count'] = len(report['scenarios'])
    report['candidate_controls'] = sum(len(item['candidates']) for item in report['scenarios'])
    report['disagreements'] = sum(not item['agrees'] for item in report['scenarios'])
    report['command_count'] = len(capture.commands)
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(
        'TERMINAL', report['scenario_count'], 'scenarios', report['candidate_controls'],
        'candidates', report['disagreements'], 'disagreements', flush=True,
    )
    accepted = (report['disagreements'] == 0 and report['candidate_controls'] == 16
                and report['all_protected_inputs_unchanged'])
    return 0 if accepted else 1


if __name__ == '__main__':
    raise SystemExit(main())
