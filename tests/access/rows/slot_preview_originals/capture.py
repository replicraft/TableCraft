#!/usr/bin/env python3
"""Capture inert original getters; optionally create all 162 fresh reference copies."""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from slot_preview_originals import common, facts, model

PROFILES = ('V2016', 'V2019')


def capture_reference(capture, java, source, before, selection, stem, directory):
    label, table, identity, column, requested, scope = selection
    target = directory / (stem + '.accdb')
    command = capture.run(java + ['update', str(source), str(target), table,
                                  str(identity), column, requested], stem + '-action')
    case = {'case': label, 'table': table, 'ID': identity, 'column': column,
            'requested': requested, 'scope': scope, 'action': command,
            'before_slot': model.slot(before[table], identity, column)}
    if target.exists():
        case['copy_before_readonly_getter'] = common.pin(target)
        case['copy_byte_identical_to_original'] = target.read_bytes() == source.read_bytes()
        case['getter'] = capture.run(java + ['oracle', str(target)], stem + '-getter')
        assert common.pin(target) == case['copy_before_readonly_getter']
        if case['getter']['status'] == 0:
            after = facts.parse(common.path(case['getter']['stdout']['path']).read_text())
            case['after_slot'] = model.slot(after[table], identity, column)
            case['same_stored_width'] = case['before_slot']['byte_width'] == case['after_slot']['byte_width']
            expected_getter = model.getter(model.value(requested))
            case['requested_value_reopened_exactly'] = case['after_slot']['getter'] == expected_getter
        case['created_copy'] = common.pin(target)
    return case


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--original', action='append', nargs=2, required=True,
                        metavar=('PROFILE', 'PATH'))
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--producer', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--references', action='store_true',
                        help='Explicitly update 162 fresh copies; never writes the originals')
    args = parser.parse_args()
    originals = {profile: common.path(path) for profile, path in args.original}
    assert set(originals) == set(PROFILES) and len(args.original) == len(PROFILES)
    jar = common.path(args.jar)
    producer = common.path(args.producer)
    report_path = common.path(args.report)
    assert not report_path.exists()
    assert common.pin(producer)['sha256'] == common.PRODUCER_SHA256
    assert common.pin(jar)['sha256'] == common.JAR_SHA256
    protected = [*originals.values(), producer, jar, Path(__file__),
                 Path(facts.__file__), Path(model.__file__), Path(common.__file__)]
    before_pins = [common.pin(path) for path in protected]
    directory = common.fresh_work(args.workdir)
    classes = directory / 'classes'
    classes.mkdir()
    capture = common.Capture(directory)
    compilation = capture.run(['javac', '-cp', str(jar), '-d', str(classes), str(producer)], 'compile')
    assert compilation['status'] == 0
    java = common.java(jar, classes)
    original_facts = []
    parsed = {}
    for profile in PROFILES:
        source = originals[profile]
        command = capture.run(java + ['oracle', str(source)], profile + '-original-getter')
        assert command['status'] == 0 and not common.path(command['stderr']['path']).read_bytes()
        getter = common.path(command['stdout']['path'])
        original_facts.append(facts.inspect(profile, source, getter))
        parsed[profile] = facts.parse(getter.read_text())
    facts_path = directory / 'original-facts.json'
    facts_path.write_text(json.dumps(original_facts, indent=2) + '\n')
    reports = []
    if args.references:
        sets = [('initial', model.reference_cases()), ('null-supplement', [
            (name + '-nonempty-null', name, 3, 'Label', 'null', 'separate-null-supplement')
            for name in ['CompactLegacy', 'RawLegacy', 'CompactModern', 'RawModern']
        ] + [('BinaryStorage-nonempty-null', 'BinaryStorage', 5, 'Bytes', 'null',
              'separate-null-supplement')])]
        for series, selections in sets:
            result = {
                'scope': ('Explicit per-row calls on fresh copies; '
                          'not native SQL or final-image batch parity.'),
                'cases': [],
            }
            for profile in PROFILES:
                for number, selection in enumerate(selections):
                    stem = profile + '-' + series + '-' + str(number)
                    case = capture_reference(capture, java, originals[profile], parsed[profile],
                                             selection, stem, directory)
                    case['profile'] = profile
                    result['cases'].append(case)
            target = directory / (series + '-references.json')
            target.write_text(json.dumps(result, indent=2) + '\n')
            reports.append((series, target))
        selectors = model.fixed_plan(json.loads(facts_path.read_text()), reports)
        plan = {'cases': selectors, 'model': common.pin(Path(model.__file__)),
                'original_facts': common.pin(facts_path),
                'scope': ('Same finite 152 plus ten NULL selectors from this capture; '
                          'reference outcomes do not determine candidate admission.')}
        (directory / 'fixed-plan.json').write_text(json.dumps(plan, indent=2) + '\n')
    after_pins = [common.pin(path) for path in protected]
    assert before_pins == after_pins
    record = {'inputs': [common.pin(path) for path in originals.values()],
              'tools': [common.pin(path) for path in [jar, producer, classes / 'SlotUpdateFixture.class']],
              'scope': 'New public-wrapper capture; does not overwrite or relabel historical execution.',
              'pins_before': before_pins, 'pins_after': after_pins,
              'original_facts': common.pin(facts_path),
              'reference_reports': [{'series': series, 'file': common.pin(path)} for series, path in reports],
              'commands': capture.commands,
              'classes': common.pin(classes / 'SlotUpdateFixture.class')}
    report_path.write_text(json.dumps(record, indent=2) + '\n')
    print('Captured', len(original_facts), 'originals;', len(reports), 'reference series')


if __name__ == '__main__':
    main()
