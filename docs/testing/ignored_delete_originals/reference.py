"""Create fresh public-writer references, or verify retained captures without writes."""

import argparse
import json
from pathlib import Path
import subprocess

from getters import parse_getters
from support import (GETTER_CLASS_FILE, JAR_SHA256, PROCESS_TIMEOUT_SECONDS,
                     REPOSITORY_ROOT, canonical, fresh_report, oracle_command, pin, verify, wire)

PROFILES = ['V2016', 'V2019']
TABLES = {
    'DeleteStorage': ['Indexed', 'Unindexed', 'Sentinel'],
    'Ignore701': ['Ignored'],
    'Coverage': ['UniquePartial', 'TupleGuid', 'TupleDecimal',
                 'ScalarTEXT0', 'ScalarTEXT1', 'ScalarBINARY'],
}
FORMAT_OFFSET = 20
SUBVERSION_RESERVED_RANGE = slice(21, 24)
ENCODING_KEY_RANGE = slice(62, 66)
PLAIN_ENCODING_KEY = bytes.fromhex('fb8abc4e')
FILE_SIGNATURE = bytes([0, 1, 0, 0])
EXPECTED_REFERENCES = 50

EXPECTED_SOURCES = {
    ('DeleteStorage', 'V2016'): '43989b6479d6f0303d06c1edca562f33fc94ee3081330acadc8f2867d7269efe',
    ('DeleteStorage', 'V2019'): '8c3d88e45874bc4ce7f93c1f1a90880a9d6e2474127d9ef359f02e4b18b371ce',
    ('Ignore701', 'V2016'): '1b7afd89440039b877ae55e700dd07c0bb0e0ed7fab6a5909aaf9073f3489a16',
    ('Ignore701', 'V2019'): '6246f75593d6ca7a7b9f71f7009cbf1627d8b50eef9504f6df14fba8b9122898',
    ('Coverage', 'V2016'): '90c5fc95e47be4996924ac6b0fe232a5209a12689527c5c05be593ec0b7f08df',
    ('Coverage', 'V2019'): '2db1bd079e3f9df28dc82c2d3eb9dc82753a8a75273b67213eefa5e2ce872b05',
}


def selections(family, tables):
    if family == 'DeleteStorage':
        return [('Indexed', 'null-only', [3, 4]), ('Indexed', 'eligible-only', [2]),
                ('Indexed', 'mixed', [2, 3]), ('Indexed', 'all-group-eligible', [1, 2, 5, 6])]
    if family == 'Ignore701':
        eligible = tables['Ignored']['indexes']['IgnoredAsc']['eligible_ids']
        assert len(eligible) == 637
        return [('Ignored', 'both-null', [0]), ('Ignored', 'key-null', [11]),
                ('Ignored', 'unique-null', [13]), ('Ignored', 'mixed', [0, 1, 11, 13]),
                ('Ignored', 'all-key-eligible', eligible)]
    result = [('UniquePartial', 'null-only', [0, 1]),
              ('UniquePartial', 'partial-null', [2, 3]),
              ('UniquePartial', 'mixed', [0, 2, 4]),
              ('UniquePartial', 'all-eligible', [2, 3, 4, 5])]
    for name in ['TupleGuid', 'TupleDecimal']:
        table = tables[name]
        eligible = table['indexes']['Ignored']['eligible_ids']
        omitted = sorted(set(table['rows']) - set(eligible))
        columns = [column for column, ascending in table['indexes']['Ignored']['columns']]
        partial = [identifier for identifier in eligible
                   if any(table['rows'][identifier]['values'][column] == 'NULL' for column in columns)]
        assert len(omitted) == 16 and len(eligible) == 185
        result.extend([(name, 'null-only', omitted), (name, 'partial-null', partial[:2]),
                       (name, 'all-eligible', eligible)])
    for name in ['ScalarTEXT0', 'ScalarTEXT1', 'ScalarBINARY']:
        table = tables[name]
        eligible = table['indexes']['Ignored']['eligible_ids']
        omitted = sorted(set(table['rows']) - set(eligible))
        assert len(omitted) == 19 and len(eligible) == 182
        result.extend([(name, 'null-only', omitted), (name, 'all-eligible', eligible)])
    return result


def check_header(raw, profile):
    assert raw[:len(FILE_SIGNATURE)] == FILE_SIGNATURE
    assert raw[FORMAT_OFFSET] == (5 if profile == 'V2016' else 6)
    assert raw[SUBVERSION_RESERVED_RANGE] == bytes(3)
    assert raw[ENCODING_KEY_RANGE] == PLAIN_ENCODING_KEY


def run(argv, workdir, label):
    stdout, stderr = workdir / (label + '.tsv'), workdir / (label + '.stderr')
    with stdout.open('xb') as out, stderr.open('xb') as err:
        result = subprocess.run(argv, stdout=out, stderr=err, timeout=PROCESS_TIMEOUT_SECONDS)
    command = {'argv': argv, 'exit_code': result.returncode,
               'stdout': pin(stdout), 'stderr': pin(stderr)}
    assert result.returncode == 0, command
    return command


def check_reference(original, actual, table_name, identifiers, before, after):
    selected = set(identifiers)
    old, new = original[table_name], actual[table_name]
    expected_rows = {identifier: row for identifier, row in old['rows'].items()
                     if identifier not in selected}
    assert canonical(new['rows']) == canonical(expected_rows)
    assert new['count'] == old['count'] - len(selected)
    transitions = []
    for name, previous in old['indexes'].items():
        current = new['indexes'][name]
        effective = selected.intersection(previous['eligible_ids'])
        assert current['order'] == [identifier for identifier in previous['order']
                                    if identifier not in selected]
        live_refs = {(row['page'], row['slot']) for row in new['rows'].values()}
        assert current['keys'] == [entry for entry in previous['keys']
                                   if (entry[1], entry[2]) in live_refs]
        for field in ['physical', 'root', 'usage', 'historical']:
            assert current[field] == previous[field]
        unchanged = all(wire.page(before, page) == wire.page(after, page)
                        for page in previous['owned'])
        if not effective:
            assert unchanged
        transitions.append({
            'index': name, 'physical': previous['physical'], 'root': previous['root'],
            'ignore_nulls': previous['ignore_nulls'], 'columns': previous['columns'],
            'original_entries': previous['entries'], 'effective_deleted': len(effective),
            'remaining_entries': current['entries'], 'owned_before': previous['owned'],
            'owned_after': current['owned'], 'height_before': previous['height'],
            'height_after': current['height'], 'pages_byte_identical': unchanged,
            'effective_empty': not effective,
            'index_empty_with_live_table_rows': not current['entries'] and bool(new['rows']),
        })
    for other in original:
        if other != table_name:
            assert canonical(actual[other]) == canonical(original[other])
    return transitions


def verify_existing(args):
    report = json.loads(args.verify_existing.read_text())
    assert report['references'] == EXPECTED_REFERENCES
    originals = {}
    expected_cases = {}
    for item in report['originals']:
        verify(item['input'])
        family, profile = item['family'], item['profile']
        assert item['input']['sha256'] == EXPECTED_SOURCES[(family, profile)]
        raw = Path(item['input']['path']).read_bytes()
        check_header(raw, profile)
        command = report['commands'][item['getter_command']]
        verify(command['stdout'])
        tables = parse_getters(Path(command['stdout']['path']), raw)
        assert canonical(tables) == canonical(item['tables'])
        originals[(family, profile)] = (raw, tables)
        for name, selection, identifiers in selections(family, tables):
            expected_cases[(family, profile, name, selection)] = identifiers

    assert len(expected_cases) == EXPECTED_REFERENCES
    for case in report['results']:
        for field in ['source', 'reference', 'request', 'getter']:
            verify(case[field])
        key = (case['family'], case['profile'], case['table'], case['selection'])
        assert case['selected_ids'] == expected_cases.pop(key)
        before, original = originals[(case['family'], case['profile'])]
        after = Path(case['reference']['path']).read_bytes()
        actual = parse_getters(Path(case['getter']['path']), after)
        transitions = check_reference(original, actual, case['table'], case['selected_ids'], before, after)
        assert canonical(transitions) == canonical(case['transitions'])
    assert not expected_cases
    for command in report['commands']:
        assert command['exit_code'] == 0
        verify(command['stdout'])
        verify(command['stderr'])
    fresh_report(args.report, {
        'scope': 'Read-only public-wrapper verification of existing fifty reference copies/captures',
        'source_report': pin(args.verify_existing), 'references': EXPECTED_REFERENCES,
        'registered_originals_unchanged': True, 'new_copies_or_java_invocations': 0,
        'all_original_reference_request_getter_command_pins_verified': True,
        'public_source': pin(Path(__file__)),
    })
    print('verified existing references', EXPECTED_REFERENCES, flush=True)


def create(args):
    assert args.workdir is not None and not args.workdir.exists()
    sources = {(family, profile): Path(path) for family, profile, path in args.input}
    assert set(sources) == set(EXPECTED_SOURCES)
    args.workdir.mkdir(parents=True, exist_ok=False)
    destinations = args.workdir / 'references'
    destinations.mkdir()
    commands, originals, results = [], [], []
    for family, selected_tables in TABLES.items():
        for profile in PROFILES:
            source = sources[(family, profile)]
            source_pin = pin(source)
            assert source_pin['sha256'] == EXPECTED_SOURCES[(family, profile)]
            before = source.read_bytes()
            check_header(before, profile)
            label = family + '-' + profile + '-registered-before'
            command = run(oracle_command(args, source, selected_tables), args.workdir, label)
            commands.append(command)
            original = parse_getters(Path(command['stdout']['path']), before)
            originals.append({
                'family': family, 'profile': profile, 'input': source_pin,
                'plain_subversion_zero': True, 'tables': original,
                'getter_command': len(commands) - 1,
            })
            for table, selection, identifiers in selections(family, original):
                label = f'{len(results):02}-{profile}-{table}-{selection}'
                request = destinations / (label + '.ids.tsv')
                with request.open('x') as output:
                    output.write(''.join(f'{identifier}\n' for identifier in identifiers))
                target = destinations / (label + '.accdb')
                getter_base = oracle_command(args, source, selected_tables)[:4]
                argv = getter_base + ['reference', str(source), str(target), table, str(request)]
                command = run(argv, args.workdir, label + '-copy')
                commands.append(command)
                reference_index = len(commands) - 1
                getter = run(oracle_command(args, target, selected_tables), args.workdir, label + '-getters')
                commands.append(getter)
                after = target.read_bytes()
                actual = parse_getters(Path(getter['stdout']['path']), after)
                transitions = check_reference(original, actual, table, identifiers, before, after)
                assert pin(source) == source_pin
                results.append({
                    'family': family, 'profile': profile, 'table': table, 'selection': selection,
                    'selected_ids': identifiers, 'source': source_pin, 'reference': pin(target),
                    'request': pin(request), 'reference_command': reference_index,
                    'getter_command': len(commands) - 1, 'getter': getter['stdout'],
                    'deleted': len(identifiers), 'remaining': actual[table]['count'],
                    'transitions': transitions,
                })
                print(label, 'deleted', len(identifiers), flush=True)
    assert len(results) == EXPECTED_REFERENCES
    fresh_report(args.report, {
        'scope': 'Existing registered originals/public copy-delete; no candidate or native Access execution',
        'helper': pin(args.helper_source), 'runner': pin(Path(__file__)), 'jar': pin(args.jar),
        'commands': commands, 'originals': originals, 'results': results,
        'references': len(results), 'original_hashes_unchanged': True,
    })


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', action='append', nargs=3, default=[], metavar=('FAMILY', 'PROFILE', 'PATH'))
    parser.add_argument('--verify-existing', type=Path)
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--helper-source', type=Path, required=True)
    parser.add_argument('--helper-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    args = parser.parse_args()
    assert Path.cwd().resolve() == REPOSITORY_ROOT
    for name in ['report', 'helper_source', 'helper_dir', 'jar', 'verify_existing', 'workdir']:
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, Path(value.resolve().relative_to(REPOSITORY_ROOT).as_posix()))
    args.input = [(family, profile, str(Path(path).resolve().relative_to(REPOSITORY_ROOT)))
                  for family, profile, path in args.input]
    assert not args.report.exists()
    assert pin(args.jar)['sha256'] == JAR_SHA256
    assert args.helper_source.is_file() and (args.helper_dir / GETTER_CLASS_FILE).is_file()
    if args.verify_existing:
        assert not args.input and args.workdir is None
        verify_existing(args)
    else:
        assert len(args.input) == len(EXPECTED_SOURCES)
        create(args)


if __name__ == '__main__':
    main()
