"""Separate immutable post-execution source/tool checks; never rewrites raw reports."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from integral_split_originals.capture_sources import pin, require_pin


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', type=Path, required=True)
    args = parser.parse_args()
    freeze = json.loads(args.freeze.read_text())
    output = ROOT / freeze['work'] / 'post-execution-pins.json'
    assert not output.exists(), 'Never overwrite an earlier postcheck'
    report = ROOT / freeze['work'] / 'report.json'
    before = pin(report)
    checked = freeze['sources'] + [freeze['jar'], freeze['baseline'], freeze['baseline_receipt']]
    for expected in checked:
        require_pin(expected)
    assert pin(report) == before
    output.write_text(json.dumps({'freeze': pin(args.freeze), 'raw_report': before,
                                 'checked_unchanged': checked, 'raw_report_unchanged': True}, indent=2) + '\n')
    print('Read-only postcheck passed; raw report unchanged.')


if __name__ == '__main__':
    main()
