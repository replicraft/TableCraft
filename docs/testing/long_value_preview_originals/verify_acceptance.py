"""Recheck revised raw60 images and cached getters; never invoke native or JVM tools."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
import capture_long_value_updates as evidence

REPORT = '.cache/ace16-long-value-acceptance-revised-first-20261027/report.json'
REPORT_HASH = '1924add4c6e4fde5a8cdd31e0ec5a5c9cc2b9c09008c20e0a1714b66e89f47c1'
PLAN = '.cache/ace16-long-value-acceptance-next-20261027/plan.json'
PLAN_HASH = 'bbcd7ad0a543b51a02f3e3ef4406eb5e1a6770a5578e8cc0a4d26e68db68fd08'
MODEL_HASH = 'b35b1aa9febd62a10e752a4826b415c1984bbd027e1e44581ba520bb5a5aa3ca'


def pin(path):
    data = (ROOT / path).read_bytes()
    return {'path': path, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def verify(expected):
    assert pin(expected['path']) == expected, expected['path']


def verify_tree(value, pins):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            expected = {key: value[key] for key in ('path', 'bytes', 'sha256')}
            verify(expected)
            pins.add((expected['path'], expected['sha256']))
        for key, child in value.items():
            if key != 'source_pins':
                verify_tree(child, pins)
    elif isinstance(value, list):
        for child in value:
            verify_tree(child, pins)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default=REPORT)
    parser.add_argument('--plan', default=PLAN)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        raise FileExistsError(args.output)
    assert pin(args.report)['sha256'] == REPORT_HASH
    assert pin(args.plan)['sha256'] == PLAN_HASH
    report = json.loads((ROOT / args.report).read_text())
    plan = json.loads((ROOT / args.plan).read_text())
    assert plan['model']['sha256'] == MODEL_HASH
    verify(plan['model'])
    spec = importlib.util.spec_from_file_location('frozen_long_value_model', ROOT / plan['model']['path'])
    model = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(model)
    original = json.loads((ROOT / plan['source_report']['path']).read_text())
    facts = json.loads((ROOT / plan['original_facts']['path']).read_text())
    profiles = {item['profile']: item for item in facts}
    before = json.loads((ROOT / report['before_first_call']['path']).read_text())
    pins = set()
    for value in (report, plan, before):
        verify_tree(value, pins)
    assert report['attempted'] == report['matched_candidates'] == 60
    assert report['unmet_controls'] == 0 and report['fatal'] is None
    assert report['command_count'] == len(report['commands']) == 180
    assert report['immutable_inputs_unchanged']
    for command in report['commands']:
        assert command['status'] == 0 and not command['timed_out']
    for selector, observed, case in zip(plan['cases'], original['cases'], report['cases'], strict=True):
        assert case['selector'] == selector and case['complete']
        source = ROOT / selector['source']['path']
        raw = source.read_bytes()
        replacement = (ROOT / selector['requested_payload']['path']).read_bytes()
        if selector['requested_kind'] == 'null':
            replacement = None
        elif selector['column'] == 'Memo':
            replacement = replacement.decode('utf-8').encode('utf-16le')
        definition = profiles[selector['profile']]['tables']['Sentinel']['definition']
        expected, details = model.expected(raw, observed['before'], selector['column'], replacement, definition)
        assert details == selector['whole_image_expectation_if_admitted'] == case['whole_image_proof']
        candidate = ROOT / case['candidate']['path']
        assert candidate.read_bytes() == expected
        cached = evidence.snapshot(
            source, ROOT / observed['before']['getter']['path'], ROOT / observed['before']['metadata']['path'],
        )
        commands = case['getter_commands']
        actual = evidence.snapshot(
            candidate, ROOT / commands['getter']['stdout']['path'],
            ROOT / commands['metadata']['stdout']['path'],
        )
        assert json.loads(json.dumps(actual[2])) == case['after']
        assert evidence.compare(cached, actual, observed['selection']) == case['comparison']
    revision = report['build']['compiled_revision']
    sources = report['build']['source_pins']
    for expected in sources:
        command = subprocess.run(
            ['git', 'show', revision + ':' + expected['path']],
            cwd=ROOT, capture_output=True, check=True, timeout=30,
        )
        assert len(command.stdout) == expected['bytes']
        assert hashlib.sha256(command.stdout).hexdigest() == expected['sha256']
    receipt = {
        'report': pin(args.report), 'plan': pin(args.plan),
        'verified_unique_artifact_pins': len(pins), 'verified_git_objects': len(sources),
        'whole_images_and_cached_getters': 60, 'native_or_jvm_calls': 0,
        'qualification': 'Revised-artifact acceptance; original first0/60 remains distinct',
    }
    args.output.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
