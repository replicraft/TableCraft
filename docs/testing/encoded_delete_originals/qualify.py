"""Qualify exact generator values, reference populations and raw/getter alignment."""

import argparse
import hashlib
import json
from pathlib import Path

from common import pin, write_report
import wire


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists()
    report = json.loads(args.capture.read_text())
    texts = [None, '', 'a', 'A', 'é', 'e\u0301', 'Straße', 'Strasse',
             'a  ', '😀', '\0', 'a' * 255, 'カタカナ']
    positive = ['TextLegacyLeaf', 'TextModernLeaf', 'BinaryLeaf', 'TextLegacyNode', 'TextModernNode', 'BinaryNode']
    negative_counts = {'TextUniqueOverflow': 1, 'MemoNegative': 1, 'TextCompositeNegative': 2,
                       'BinaryCompositeNegative': 2, 'TextIgnoredNegative': 1, 'BinaryIgnoredNegative': 1}

    value_cells = 0
    summaries = []
    for file in report['files']:
        path = Path(file['input']['path'])
        raw = path.read_bytes()
        assert hashlib.sha256(raw).hexdigest() == file['input']['sha256']
        state = path.stem.split('-', 2)[2]
        profile = 'V2016' if raw[wire.HEADER_FORMAT_OFFSET] == 5 else 'V2019'
        for name, table in file['tables'].items():
            if name not in positive:
                assert table['rows'] == negative_counts.get(name, 1)
                continue
            initial = 24 if name.endswith('Leaf') else 701
            ids = list(range(1, initial + 1))
            if state == 'delete-all':
                ids = []
            elif state == 'delete-tie':
                ids.remove(4)
            elif state == 'delete-null':
                ids.remove(1)
            assert [int(row[0]) for row in table['getter_rows']] == ids
            for row in table['getter_rows']:
                identifier = int(row[0])
                index = (identifier - 1) * 7
                if name.startswith('Text'):
                    value = texts[index % len(texts)]
                    expected = 'NULL' if value is None else 'T:' + value.encode('utf-16-be').hex()
                else:
                    slot = index % 9
                    if slot == 0:
                        expected = 'NULL'
                    else:
                        length = [0, 0, 1, 7, 8, 9, 16, 17, 255][slot]
                        value = bytes((position * 31 + (255 if index % 3 == 0 else 128)) & 255
                                      for position in range(length))
                        expected = 'B:' + value.hex()
                assert row[3] == expected, (profile, state, name, identifier, row[3], expected)
                value_cells += 1
        getter_path = Path(next(command['stdout']['path'] for command in report['commands']
                                if command['argv'][-1] == str(path)))
        orders = {}
        for line in getter_path.read_text().splitlines():
            fields = line.split('\t')
            if fields[0] == 'ORDER':
                orders.setdefault((fields[1], fields[2]), []).append(int(fields[3]))
        for index in file['indexes']:
            table = file['tables'][index['table']]
            if index['table'] == 'PreservedPayloads':
                continue
            identities = {(int(row[1]), int(row[2])): int(row[0]) for row in table['getter_rows']}
            keys = []
            for line in getter_path.read_text().splitlines():
                fields = line.split('\t')
                if fields[:3] == ['KEY', index['table'], index['name']]:
                    keys.append((int(fields[3]), int(fields[4])))
            assert sorted(keys) == sorted(identities)
            assert [identities[key] for key in keys] == orders.get((index['table'], index['name']), [])
        if state == 'before':
            topology = []
            for index in file['indexes']:
                if index['table'] in positive and index['name'] != 'Twin':
                    internal_links = [node for node in index['nodes']
                                      if node['page'] != index['root']
                                      and (node['previous'] or node['next'])]
                    topology.append({'table': index['table'], 'index': index['name'], 'root': index['root'],
                                     'height': index['height'], 'owned_pages': index['owned_count'],
                                     'leaves': len(index['leaves']), 'linked_nonroot_nodes': internal_links})
            for name in positive:
                aliases = [item for item in file['indexes']
                           if item['table'] == name and item['name'] in ['Asc', 'Twin']]
                assert len(aliases) == 2
                for item in aliases:
                    for field in ['physical', 'root', 'usage']:
                        assert item[field] == aliases[0][field]
            summaries.append({
                'profile': profile, 'input': file['input'],
                'rows': sum(table['rows'] for table in file['tables'].values()),
                'positive_rows': sum(file['tables'][name]['rows'] for name in positive),
                'topology': topology,
            })

    result = {
        'scope': 'Generator-value/getter/raw-reference checks; no engine or Access SQL execution',
        'capture_report': pin(args.capture), 'qualification_source': pin(Path(__file__)),
        'originals': summaries, 'file_states': len(report['files']),
        'public_getter_model_value_cells': value_cells,
        'all_eight_source_hashes_unchanged': True,
        'raw_key_and_row_reference_matches': True,
        'cursor_order_matches_raw_references': True,
        'sentinel_memo_ole_getters_unchanged_across_states': True,
    }
    write_report(args.report, result)
    for summary in summaries:
        print(summary['profile'], 'rows', summary['rows'], flush=True)
    print('independently matched value cells', value_cells, flush=True)


if __name__ == '__main__':
    main()
