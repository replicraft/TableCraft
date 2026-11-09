"""Verify saved map-growth outcomes without native/JVM fixture processes."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from usage_map_growth import model

PLAN_SHA256 = '48b32bb601b1b5333e6014fc2285905e110a581c3927a4b197e61452c921699f'
PROOF_SHA256 = 'd4e913333d557ae03257e96113a426f539a4de92b63efe51c268c9cc14e131e9'
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def path(value):
    resolved = (ROOT / value).resolve()
    resolved.relative_to(ROOT)
    return resolved


def pin(value):
    target = path(value)
    raw = target.read_bytes()
    return {'path': target.relative_to(ROOT).as_posix(), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


def canonical(value):
    return json.loads(json.dumps(value))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--report-sha256', required=True)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    parser.add_argument('--require-success', action='store_true')
    args = parser.parse_args()
    assert pin(args.report)['sha256'] == args.report_sha256
    assert pin(args.plan)['sha256'] == PLAN_SHA256
    assert pin(args.proof)['sha256'] == PROOF_SHA256
    report = json.loads(path(args.report).read_text())
    plan = json.loads(path(args.plan).read_text())
    specification = importlib.util.spec_from_file_location('saved_original_map_growth_proof', path(args.proof))
    proof = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(proof)
    live_pins = 0

    def verify_pins(value):
        nonlocal live_pins
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                assert pin(value['path']) == value
                live_pins += 1
            for name, entry in value.items():
                if name != 'compiled_source_pins':
                    verify_pins(entry)
        elif isinstance(value, list):
            for entry in value:
                verify_pins(entry)

    verify_pins(report)
    assert report['inputs_unchanged']
    assert report['before_pins'] == report['after_pins'] and report['pin_errors'] == []
    revision = report['binding']['compiled_revision']
    git_objects = 0
    for entry in report['compiled_source_pins']:
        raw = subprocess.check_output(['git', 'show', revision + ':' + entry['path']], cwd=ROOT, timeout=30)
        assert len(raw) == entry['bytes']
        assert hashlib.sha256(raw).hexdigest() == entry['sha256']
        git_objects += 1
    cases = {case['id']: case for case in plan['cases']}
    assert len(cases) == 2
    assert len({case['id'] for case in report['cases']}) == len(report['cases'])
    assert set(case['id'] for case in report['cases']).issubset(cases)
    assert len(report['commands']) <= 4
    accepted = 0
    inspected = 0
    observations = []
    for result in report['cases']:
        case = cases[result['id']]
        original = path(case['source']['path'])
        expected, facts = proof.expected_image(case, original.read_bytes())
        assert facts == case['expected_image']
        candidate = result['candidate']
        image_match = False
        if candidate is not None:
            image_match = path(candidate['path']).read_bytes() == expected
        if result['agrees']:
            assert candidate is not None and image_match
            assert result['whole_image_proof'] == facts
            native = next(command for command in report['commands'] if command['label'] == case['id'] + '-native')
            getter = next(command for command in report['commands'] if command['label'] == case['id'] + '-inspect')
            for command in (native, getter):
                assert command['status'] == 0 and command['stopped_for'] is None
            assert path(native['stdout']['path']).read_text() == 'candidate_changed_rows=1\n'
            assert path(native['stderr']['path']).read_text() == NOTICE
            assert path(getter['stderr']['path']).read_bytes() == b''
            before = model.snapshot(original, path(case['cached_getter']['path']), 'before', case['profile'])
            after = model.snapshot(path(candidate['path']), path(getter['stdout']['path']), 'after', case['profile'])
            assert canonical(after) == result['typed_index_map_snapshot']
            assert canonical(model.transition(before, after)) == result['transition']
            accepted += 1
            inspected += 1
        else:
            assert 'error' in result or not report['inputs_unchanged']
        observations.append({
            'id': case['id'], 'recorded_agreement': result['agrees'],
            'candidate_present': candidate is not None,
            'candidate_matches_independent_image': image_match,
        })
    assert report['completed'] == accepted
    assert report['unmet'] == 2 - accepted
    success = accepted == 2 and len(report['commands']) == 4
    if args.require_success:
        assert success
    print(json.dumps({
        'scope': 'Saved outcomes/images/getters only; zero native/JVM fixture calls',
        'raw_report': pin(args.report), 'attempted_cases': len(report['cases']),
        'accepted_cases': accepted, 'unmet': report['unmet'],
        'saved_commands': len(report['commands']), 'complete_cached_getters': inspected,
        'recursive_live_pin_occurrences': live_pins, 'historical_git_objects': git_objects,
        'success': success, 'cases': observations,
    }, indent=2))


if __name__ == '__main__':
    main()
