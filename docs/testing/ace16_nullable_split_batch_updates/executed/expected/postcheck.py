"""Second SOURCE/request replay, without importing the image constructor."""
import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OUTPUT = ROOT / '.cache/ace16-nullable-split-batch-expected-first-20261029'
PAGE = 4096
CAPACITY = 3616


def pin(path):
    path = Path(path)
    raw = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(raw),
            'sha256': hashlib.sha256(raw).hexdigest()}


def require(item):
    assert pin(ROOT / item['path']) == item, item['path']


def put(raw, offset, value, width):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def measure(entries, old_prefix):
    common = 0
    if entries:
        common = min(len(entry) for entry in entries)
        for position in range(common):
            if any(entry[position] != entries[0][position] for entry in entries):
                common = position
                break
    prefix = min(old_prefix, common)
    active = sum(len(entry) for entry in entries) - max(0, len(entries) - 1) * prefix
    if active > CAPACITY:
        prefix = common
        active = sum(len(entry) for entry in entries) - max(0, len(entries) - 1) * prefix
    return prefix, active


def page(original, entries, owner, *, kind=4, previous=0, next_=0, tail=0):
    result = bytearray(PAGE) if original is None else bytearray(original)
    old_prefix = 0 if original is None or kind == 3 else int.from_bytes(original[24:26], 'little')
    prefix, active = measure(entries, old_prefix)
    assert active <= CAPACITY
    result[0] = kind
    if original is None:
        result[1] = 1
    put(result, 2, CAPACITY - active, 2)
    put(result, 4, owner, 4)
    put(result, 12, previous, 4)
    put(result, 16, next_, 4)
    put(result, 20, tail, 4)
    put(result, 24, prefix, 2)
    result[26] = int(kind == 3)
    result[27:480] = bytes(453)
    end = 0
    for ordinal, entry in enumerate(entries):
        suffix = entry if ordinal == 0 else entry[prefix:]
        result[480 + end:480 + end + len(suffix)] = suffix
        end += len(suffix)
        result[27 + end // 8] |= 1 << (end % 8)
    assert end == active
    return result


def frame(raw, root):
    page, slot = root >> 8, root & 255
    base = page * PAGE
    word = int.from_bytes(raw[base + 14 + slot * 2:base + 16 + slot * 2], 'little')
    assert word & 0xe000 == 0
    start = word & 0x1fff
    end = PAGE if slot == 0 else int.from_bytes(raw[base + 12 + slot * 2:base + 14 + slot * 2], 'little') & 0x1fff
    return base + start, base + end


def derive(source, case, before):
    original = before['raw']
    expected = bytearray(original)
    table = before['facts']['tables'][case['table']]
    rows = {row['id']: {'root': row['root'], 'values': dict(row['values'])} for row in table['rows']}
    columns = {column['name']: column for column in table['columns']}
    changed = set()
    cells = 0
    assignments = []
    ordinals = {row['id']: position for position, row in enumerate(table['rows'])}
    for identity, updates in case['updates']:
        row = rows[identity]
        start, end = frame(original, row['root'])
        for name, value in updates.items():
            column = columns[name]
            metadata = bytes.fromhex(column['raw_hex'])
            offset = start + 2 + int.from_bytes(metadata[21:23], 'little')
            bit = 1 << column['number']
            previous = row['values'][name]
            assignments.append({'table': case['table'], 'column': name, 'ordinal': ordinals[identity],
                                'identity': identity, 'logical_reference': row['root'],
                                'expected': previous, 'value': value})
            if previous != value:
                cells += 1
                changed.add(row['root'])
            if value is not None:
                expected[offset:offset + 4] = value.to_bytes(4, 'little', signed=True)
            expected[end - 1] = (expected[end - 1] & ~bit) | (bit if value is not None else 0)
            row['values'][name] = value
    allocations = []
    touched_maps = set()
    histories = []
    for physical, name in [(0, 'LeftOrder'), (1, 'PairOrder')]:
        index = table['indexes'][name]
        ignored = bool(index['flags'] & 2)
        pairs = []
        old_families = set()
        incoming = set()
        for original_row in table['rows']:
            current = rows[original_row['id']]
            old_values = tuple(original_row['values'][column] for column, _ in index['columns'])
            new_values = tuple(current['values'][column] for column, _ in index['columns'])
            if not all(value is None for value in old_values):
                old_families.add(old_values)
            if ignored and all(value is None for value in old_values) and not all(value is None for value in new_values):
                incoming.add(new_values)
            if ignored and all(value is None for value in new_values):
                continue
            key = b''.join(source.previous.component(current['values'][column], ascending)
                           for column, ascending in index['columns'])
            pairs.append((key, current['root']))
        pairs.sort()
        complete = [key + root.to_bytes(4, 'big') for key, root in pairs]
        root = index['root']
        source_page = original[root * PAGE:(root + 1) * PAGE]
        if measure(complete, int.from_bytes(source_page[24:26], 'little'))[1] <= CAPACITY:
            root_page = page(source_page, complete, table['definition'])
        else:
            middle = len(complete) // 2
            right, left = len(expected) // PAGE, len(expected) // PAGE + 1
            expected.extend(bytes(PAGE * 2))
            expected[right * PAGE:(right + 1) * PAGE] = page(None, complete[middle:], table['definition'], previous=left)
            expected[left * PAGE:(left + 1) * PAGE] = page(None, complete[:middle], table['definition'], next_=right)
            root_page = page(source_page, [complete[middle - 1] + left.to_bytes(4, 'big')], table['definition'], kind=3, tail=right)
            role = 'Table:' + str(table['definition']) + ':IndexData:' + str(physical)
            for number in [right, left]:
                allocations.append({'physical': physical, 'page': number, 'purpose': 'INDEX'})
                for mapping, set_bit in [(before['facts']['maps'][role], True), (before['facts']['maps']['GlobalFree'], False)]:
                    bit = source.future(original, mapping, number)
                    assert bit is not None and bit['set'] != set_bit
                    offset, mask = bit['offset'], bit['mask']
                    expected[offset] = expected[offset] | mask if set_bit else expected[offset] & ~mask
                    touched_maps.add(offset)
        expected[root * PAGE:(root + 1) * PAGE] = root_page
        delta = len(incoming - old_families) if ignored else 0
        histories.append(index['history'] + delta)
        put(expected, table['definition'] * PAGE + index['history_offset'], index['history'] + delta, 4)
    return bytes(expected), {'changed_roots': sorted(changed), 'changed_cells': cells,
                             'assignments': assignments, 'histories': histories,
                             'allocations': allocations, 'map_offsets': sorted(touched_maps)}


def main():
    output_freeze = pin(OUTPUT / 'freeze.json')
    assert output_freeze['sha256'] == 'ee9f402cb997918905d1aa071fb5ddcb1cc68b256773e44f4ce6a1c81b039583'
    frozen = json.loads((OUTPUT / 'freeze.json').read_text())
    for item in frozen['inputs'] + frozen['outputs']:
        require(item)
    specification = importlib.util.spec_from_file_location('nullable_source_postcheck', ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029/source.py')
    source = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(source)
    requests = json.loads((HERE / 'requests.json').read_text())['cases']
    gate = json.loads((ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029/source-gates-first.json').read_text())
    before = {}
    for case in gate['facts']:
        facts = source.inspect(ROOT / case['source']['path'], ROOT / case['getter']['path'], case['profile'], case['direction'])
        assert json.loads(json.dumps(facts)) == case['facts']
        before[case['id']] = {'raw': (ROOT / case['source']['path']).read_bytes(), 'facts': facts}
    results = []
    for case in requests:
        expected, details = derive(source, case, before[case['source_id']])
        image_path = OUTPUT / (case['id'] + '.accdb')
        actual = image_path.read_bytes()
        assert actual == expected, (case['id'], 'independent complete-image replay')
        recorded = json.loads((OUTPUT / (case['id'] + '.json')).read_text())
        for name in ['changed_roots', 'changed_cells', 'assignments', 'allocations', 'map_offsets']:
            assert recorded[name] == details[name], (case['id'], name)
        assert [item['history_after'] for item in recorded['physical']] == details['histories']
        results.append({'case': case['id'], 'image': pin(image_path), 'bytes_checked': len(actual),
                        'changed_roots': len(details['changed_roots']), 'changed_cells': details['changed_cells'],
                        'histories': details['histories'], 'INDEX_pages': len(details['allocations'])})
    for item in frozen['inputs'] + frozen['outputs']:
        require(item)
    require(output_freeze)
    receipt = {'verified': True, 'source_targets': 8, 'sentinels': 4, 'complete_images': len(results),
               'bytes_checked': sum(item['bytes_checked'] for item in results), 'children': 0,
               'constructor_imported': False, 'output_freeze': output_freeze, 'results': results,
               'checks': 'Independent SOURCE/request full records, exact masks/opaque NULL payloads, logical ROWs, complete root/leaf framing and protected bytes, historical counters and cumulative map cells; all original/import/output pins exact.'}
    target = HERE / 'postcheck-first.json'
    with target.open('x') as handle:
        handle.write(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({'verified': True, 'images': len(results), 'bytes_checked': receipt['bytes_checked'], 'receipt': pin(target)}))


if __name__ == '__main__':
    main()
