"""Requalify every BEFORE state before constructing any of twenty images."""
import argparse
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCE = ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029'


def pin(path):
    path = Path(path)
    raw = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def require(expected):
    assert pin(ROOT / expected['path']) == expected, expected['path']


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def normalized(value):
    return json.loads(json.dumps(value))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--static-only', action='store_true')
    parser.add_argument('--output', type=Path, default=ROOT / '.cache/ace16-nullable-split-batch-expected-first-20261029')
    args = parser.parse_args()
    freeze_path = HERE / 'freeze.json'
    freeze_pin = pin(freeze_path)
    freeze = json.loads(freeze_path.read_text())
    for item in freeze['inputs']:
        require(item)
    source_gate = json.loads((SOURCE / 'source-gates-first.json').read_text())
    assert source_gate['source_qualification'] is True
    assert source_gate['source_gates'] == 8 and source_gate['sentinel_gates'] == 4
    assert source_gate['constructor_images'] == source_gate['actual_child_commands'] == 0
    requests = json.loads((HERE / 'requests.json').read_text())
    assert requests['case_count'] == len(requests['cases']) == 20
    assert requests['row_updates'] == sum(len(case['updates']) for case in requests['cases']) == 8008
    assert len({case['id'] for case in requests['cases']}) == 20
    assert len(source_gate['facts']) == 4
    source_cases = {case['id']: case for case in source_gate['facts']}
    expected_order = [('Items', 'Full'), ('Items', 'Clear'), ('UniqueItems', 'Full'),
                      ('UniqueItems', 'Clear'), ('Items', 'DuplicateFamily')]
    for source_id, source_case in source_cases.items():
        cases = [case for case in requests['cases'] if case['source_id'] == source_id]
        assert [(case['table'], case['recipe']) for case in cases] == expected_order
        for case in cases:
            assert case['profile'] == source_case['profile'] and case['direction'] == source_case['direction']
            assert case['id'] == source_id + '-' + case['table'] + '-' + case['recipe']
    for filename in ['model.py', 'check.py', 'prepare.py']:
        ast.parse((HERE / filename).read_text(), filename=filename)
    if args.static_only:
        print(json.dumps({'static': True, 'input_pins': len(freeze['inputs']),
                          'cases': 20, 'row_requests': 8008, 'images_constructed': 0, 'children': 0}))
        return
    args.output.mkdir(exist_ok=False)
    report = {'completed': False, 'source_gates_before_images': False,
              'model_derivations': 0, 'actual_child_commands': 0,
              'attempted_derivations': 0, 'phase': 'source-replay',
              'input_freeze': freeze_pin, 'inputs': freeze['inputs'], 'results': []}
    try:
        source = load('nullable_source', SOURCE / 'source.py')
        constructor = load('nullable_engineering_model', HERE / 'model.py')
        check = load('nullable_engineering_check', HERE / 'check.py')
        known_inputs = {item['path'] for item in freeze['inputs']}
        for module in list(sys.modules.values()):
            filename = getattr(module, '__file__', None)
            if filename:
                path = Path(filename).resolve()
                if path.is_relative_to(ROOT) and path.suffix == '.py':
                    assert str(path.relative_to(ROOT)) in known_inputs, str(path)
        before = {}
        for case in source_gate['facts']:
            require(case['source'])
            require(case['getter'])
            facts = source.inspect(ROOT / case['source']['path'], ROOT / case['getter']['path'],
                                   case['profile'], case['direction'])
            assert normalized(facts) == case['facts'], ('SOURCE replay', case['id'])
            before[case['id']] = {'raw': (ROOT / case['source']['path']).read_bytes(), 'facts': normalized(facts)}
        assert len(before) == 4
        report['source_gates_before_images'] = True
        source_receipt = {'qualified_targets': 8, 'qualified_sentinels': 4,
                          'images_constructed': 0, 'source_pins': [item['source'] for item in source_gate['facts']]}
        (args.output / 'sources-before-construction.json').write_text(json.dumps(source_receipt, indent=2) + '\n')
        for case in requests['cases']:
            for item in freeze['inputs']:
                require(item)
            require(freeze_pin)
            report['phase'] = 'construction'
            report['active_case'] = case['id']
            report['attempted_derivations'] += 1
            image, details = constructor.derive(case, before[case['source_id']], case['updates'])
            assert len(image) <= 8 * 1024 * 1024
            proof = check.certify(case, before[case['source_id']], image, details)
            destination = args.output / (case['id'] + '.accdb')
            details_path = args.output / (case['id'] + '.json')
            with destination.open('xb') as output:
                output.write(image)
            with details_path.open('x') as output:
                output.write(json.dumps(dict(details, raw_readback=proof), indent=2) + '\n')
            report['results'].append({'case': case['id'], 'expected': pin(destination),
                                      'details': pin(details_path), 'proof': proof})
            report['model_derivations'] += 1
        assert report['model_derivations'] == 20
        report['phase'] = 'complete'
        report['completed'] = True
    except BaseException as error:
        report['failure'] = {'type': type(error).__name__, 'message': str(error)}
        raise
    finally:
        report['pin_errors'] = []
        for item in freeze['inputs'] + [freeze_pin]:
            try:
                require(item)
            except BaseException:
                report['pin_errors'].append(item['path'])
        if report['pin_errors']:
            report['completed'] = False
        report_path = args.output / 'report.json'
        report_path.write_text(json.dumps(report, indent=2) + '\n')
        outputs = [pin(path) for path in sorted(args.output.iterdir()) if path.is_file()]
        (args.output / 'freeze.json').write_text(json.dumps({
            'inputs': freeze['inputs'] + [freeze_pin], 'outputs': outputs,
            'completed': report['completed'], 'derivations': report['model_derivations'],
        }, indent=2) + '\n')
    assert not report['pin_errors']
    print(json.dumps({'completed': True, 'expected_images': 20,
                      'source_gates': 8, 'sentinel_gates': 4, 'children': 0,
                      'output_freeze': pin(args.output / 'freeze.json')}))


if __name__ == '__main__':
    main()
