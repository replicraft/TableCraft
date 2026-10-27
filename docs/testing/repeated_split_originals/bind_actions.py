"""Bind the fixed four MAX groups to a successful fresh source reproduction."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
from integral_split_originals.capture_sources import pin, require_pin
from repeated_split_originals import capture_actions, capture_sources


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-plan', type=Path, required=True)
    parser.add_argument('--source-report', type=Path, required=True)
    parser.add_argument('--source-postcheck', type=Path, required=True)
    parser.add_argument('--work', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists(), 'Never replace an earlier action plan'
    work = (ROOT / args.work).resolve()
    work.relative_to(ROOT / '.cache')
    assert not work.exists(), 'A fresh action work directory is required'
    source_plan = json.loads(args.source_plan.read_text())
    capture_sources.validate(source_plan)
    report = json.loads(args.source_report.read_text())
    postcheck = json.loads(args.source_postcheck.read_text())
    assert postcheck['raw_report'] == pin(args.source_report)
    assert postcheck['freeze'] == pin(args.source_plan)
    assert postcheck['raw_report_unchanged']
    for expected in postcheck['checked_unchanged']:
        require_pin(expected)
    assert report['status'] == 'all source gates and four exact no-op admissions passed'
    assert len(report['profiles']) == 2 and len(report['commands']) == 9
    plan = {
        'scope': 'Four unchanged zero-to-MAX target/profile groups; one prior attempt, public copy and complete description each.',
        'work': work.relative_to(ROOT).as_posix(),
        'limits': source_plan['limits'],
        'execution_driver': pin(BASE / 'capture_actions.py'),
        'sources': source_plan['sources'],
        'jar': source_plan['jar'],
        'baseline': source_plan['baseline'],
        'baseline_receipt': source_plan['baseline_receipt'],
        'compiled_revision': source_plan['compiled_revision'],
        'source_report': pin(args.source_report),
        'source_postcheck': pin(args.source_postcheck),
        'source_freeze': pin(args.source_plan),
        'static_checks': [],
        'derived_from': [pin(args.source_plan)],
        'java_sources': source_plan['java_sources'] + [
            (BASE / 'RepeatedSplitUpdateReference.java').relative_to(ROOT).as_posix(),
        ],
        'originals': [],
    }
    for original in report['profiles']:
        require_pin(original['original'])
        assert set(original['facts']['tables']) == {'Int64Keys', 'CurrencyKeys'}
        plan['originals'].append({
            'profile': original['profile'],
            'input': original['original'],
            'selectors': {
                table: {'identity': 1, 'original_ordinal': facts['selected_ordinal'],
                        'original_reference': facts['selected_reference'],
                        'expected_units': 0, 'kind': facts['kind']}
                for table, facts in original['facts']['tables'].items()
            },
        })
    plan['process_inventory'] = capture_actions.inventory(plan)
    assert len(plan['process_inventory']) == 13
    args.output.write_text(json.dumps(plan, indent=2) + '\n')
    print('Bound thirteen fixed actions; no JVM/native process executed.')


if __name__ == '__main__':
    main()
