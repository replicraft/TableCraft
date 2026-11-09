"""Verify four accepted recursive split images from pinned cached evidence only."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORT = '.cache/ace16-parent-four-acceptance-first-20261027/report.json'
REPORT_HASH = '332a3ee6d3e2f23d9225ba5c67e918c33a7bf66af9724f357364cc53522484eb'
RUNNER = '.cache/ace16-parent-four-acceptance-next-20261027/replay.py'
RUNNER_HASH = '838275e355a0289afac98a8ef2240b0495c175a82a60741666d7557f89fae47d'


def pin(path):
    data = (ROOT / path).read_bytes()
    return {'path': path, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def verify_tree(value, verified):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            expected = {key: value[key] for key in ('path', 'bytes', 'sha256')}
            assert pin(expected['path']) == expected, expected['path']
            verified.add((expected['path'], expected['sha256']))
        for key, child in value.items():
            if key != 'source_pins':
                verify_tree(child, verified)
    elif isinstance(value, list):
        for child in value:
            verify_tree(child, verified)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise FileExistsError(args.output)
    assert pin(args.report)['sha256'] == REPORT_HASH
    assert pin(RUNNER)['sha256'] == RUNNER_HASH
    spec = importlib.util.spec_from_file_location('frozen_recursive_split_runner', ROOT / RUNNER)
    runner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(runner)
    report = json.loads((ROOT / args.report).read_text())
    assert report['attempted'] == report['matched'] == 4 and report['unmet'] == 0
    assert report['command_count'] == len(report['commands']) == 8
    assert report['fatal'] is None and report['immutable_inputs_unchanged']
    verified = set()
    verify_tree(report, verified)
    before = json.loads((ROOT / report['before_first_call']['path']).read_text())
    verify_tree(before, verified)
    selectors, models, _ = runner.read_inputs()
    receipt = report['build']
    gate_args = argparse.Namespace(
        artifact=Path(receipt['artifact']['path']),
        artifact_sha256=receipt['artifact']['sha256'],
        revision=receipt['compiled_revision'],
        build_receipt=Path(receipt['receipt']['path']),
    )
    assert runner.gate(gate_args) == receipt
    actual_cases = {case['id']: case for case in report['cases']}
    for group in selectors['groups']:
        plan, model = models[group['namespace']]
        for selector in group['cases']:
            case = actual_cases[selector['id']]
            assert case['complete'] and 'failure' not in case
            assert selector == case['selector']
            original = plan['cases'][selector['original_case_index']]
            assert original == case['plan_case']
            source = (ROOT / original['source']['path']).read_bytes()
            expected, facts = model.expected_image(original, source)
            candidate = (ROOT / case['candidate']['path']).read_bytes()
            assert candidate == expected and facts == case['whole_image_proof']
            cached = runner.parse(source, selector['cached_source_capture']['command']['stdout']['path'])
            fresh = runner.parse(candidate, case['getter']['stdout']['path'])
            assert runner.compare(selector, cached, fresh) == case['comparison']
            assert json.loads(json.dumps(fresh[2])) == case['maps']
            assert case['native']['status'] == case['getter']['status'] == 0
    result = {
        'report': pin(args.report), 'accepted_whole_images': 4,
        'root_growth_cases': 2, 'below_root_internal_cases': 2,
        'unique_artifact_pins': len(verified),
        'historical_git_objects': len(receipt['source_pins']),
        'native_or_jvm_calls': 0,
    }
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
