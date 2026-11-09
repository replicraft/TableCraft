"""Read-only public getters and independent framing checks into fresh artifacts."""

import argparse
import json
from pathlib import Path
import subprocess

from common import (GETTER_CLASS_FILE, PROFILES, STATES, add_getter_arguments,
                    check_getter, oracle_command, pin, write_report)
import wire

GETTER_TIMEOUT_SECONDS = 45


def inspect(raw, output):
    tables, indexes, keys, payload = {}, [], {}, []
    for line in output.read_text().splitlines():
        fields = line.split('\t')
        if fields[0] == 'TABLE':
            tables[fields[1]] = {'rows': int(fields[2]),
                                 'definition': int(fields[3]), 'getter_rows': []}
        elif fields[0] == 'ROW':
            tables[fields[1]]['getter_rows'].append(fields[2:])
        elif fields[0] == 'COLUMN':
            tables[fields[1]]['column'] = fields[2:]
        elif fields[0] == 'PAYLOAD':
            payload.append(fields[1:])
        elif fields[0] == 'INDEX':
            indexes.append({
                'table': fields[1], 'name': fields[2], 'physical': int(fields[3]),
                'root': int(fields[4]), 'kind': int(fields[5]),
                'height': int(fields[6]), 'owned_count': int(fields[7]),
                'historical_count': int(fields[8]), 'counter_offset': int(fields[9]),
                'entries': int(fields[10]),
            })
        elif fields[0] == 'KEY':
            keys.setdefault((fields[1], fields[2]), []).append(
                (fields[5], int(fields[3]), int(fields[4])))

    for index in indexes:
        table = tables[index['table']]
        usage, root = wire.physical(raw, table['definition'], index['physical'])
        assert root == index['root']
        entries, leaves, nodes = wire.decode_tree(raw, root, table['definition'])
        assert entries == keys.get((index['table'], index['name']), [])
        assert len(entries) == index['entries'] == table['rows']
        owned = wire.members(raw, usage)
        assert len(owned) == index['owned_count']
        assert {item['page'] for item in leaves + nodes}.issubset(owned)
        index.update({'usage': usage, 'owned': owned, 'leaves': leaves, 'nodes': nodes})
    for name, table in tables.items():
        if name != 'PreservedPayloads':
            assert len(table['getter_rows']) == table['rows']
    return tables, indexes, payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True, help='Eight existing DB files')
    parser.add_argument('--output-dir', type=Path, required=True, help='Must not exist')
    parser.add_argument('--report', type=Path, required=True, help='Must not exist')
    add_getter_arguments(parser)
    args = parser.parse_args()
    check_getter(args)
    assert not args.report.exists()
    files = [args.corpus / f'Encoded-{profile}-{state}.accdb'
             for profile in PROFILES for state in STATES]
    original_pins = [pin(path) for path in files]
    args.output_dir.mkdir(parents=True, exist_ok=False)
    commands, results = [], []
    for path in files:
        output = args.output_dir / (path.stem + '.getters.tsv')
        errors = args.output_dir / (path.stem + '.getters.stderr')
        command = oracle_command(args, path)
        with output.open('xb') as stdout, errors.open('xb') as stderr:
            completed = subprocess.run(command, stdout=stdout, stderr=stderr,
                                       timeout=GETTER_TIMEOUT_SECONDS)
        assert completed.returncode == 0, (path, completed.returncode)
        commands.append({'argv': command, 'exit_code': completed.returncode,
                         'stdout': pin(output), 'stderr': pin(errors)})
        raw = path.read_bytes()
        tables, indexes, payload = inspect(raw, output)
        results.append({
            'input': pin(path), 'header_code': raw[wire.HEADER_FORMAT_OFFSET],
            'subversion': raw[wire.HEADER_SUBVERSION_OFFSET], 'tables': tables,
            'indexes': indexes, 'preserved_payload_hashes': payload,
            'raw_getter_keys_match': True,
        })
        print(path.name, 'rows', sum(t['rows'] for t in tables.values()),
              'indexes', len(indexes), flush=True)

    assert [pin(path) for path in files] == original_pins
    for profile in PROFILES:
        group = [item for item in results if profile in item['input']['path']]
        assert len(group) == len(STATES)
        assert len({json.dumps(item['preserved_payload_hashes'], sort_keys=True)
                    for item in group}) == 1
    report = {
        'scope': 'Public-Jackcess getters/raw framing; no engine or Access SQL execution',
        'helper': pin(args.helper_source),
        'class': pin(args.helper_dir / GETTER_CLASS_FILE), 'jar': pin(args.jar),
        'capture_source': pin(Path(__file__)), 'wire_source': pin(Path(wire.__file__)),
        'inputs_unchanged': True, 'commands': commands, 'files': results,
    }
    write_report(args.report, report)
    print('all immutable input hashes and sentinel getter hashes preserved', flush=True)


if __name__ == '__main__':
    main()
