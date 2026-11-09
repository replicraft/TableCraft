"""Public finite producer wrapper; importing/validating never starts a process."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common
from compressed_memo_observations import capture as LONG
import delete_reference_maps as MAP

DIRECTORY = Path(__file__).resolve().parent
PAGE_BYTES = 4096
MAX_FILE_BYTES = 8 * 1024 * 1024
MAX_PROCESSES = 13
SELECTED = 'IndirectLongValues'
PADDING = 'Padding'
PROFILES = ('V2016', 'V2019')
STAGES = ('after-first', 'after-padding', 'before')


def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + '\n')


def number(data, offset, width=2):
    part = data[offset:offset + width]
    assert len(part) == width
    return int.from_bytes(part, 'little')


def definition(raw, root):
    first = raw[root * PAGE_BYTES:(root + 1) * PAGE_BYTES]
    assert first[:2] == b'\x02\x01'
    length = number(first, 8, 4)
    assert 12 <= length <= 4096 + 63 * 4088
    data = bytearray(first)
    pages = [root]
    following = number(first, 4, 4)
    while len(data) < length:
        assert following not in pages and 0 < following < len(raw) // PAGE_BYTES
        assert len(pages) < 64
        pages.append(following)
        page = raw[following * PAGE_BYTES:(following + 1) * PAGE_BYTES]
        assert page[:2] == b'\x02\x01'
        following = number(page, 4, 4)
        data.extend(page[8:8 + min(length - len(data), 4088)])
    assert following == 0
    return bytes(data[:length]), pages


def declarations(raw, table):
    data, pages = definition(raw, table['definition'])
    physical = number(data, 51, 4)
    logical = number(data, 47, 4)
    columns = number(data, 45)
    assert columns == table['columns_count']
    start = 63 + physical * 12
    position = start + columns * 25
    blocks = []
    for index in range(columns):
        width = number(data, position)
        position += 2
        label = data[position:position + width].decode('utf-16le')
        position += width
        block = data[start + index * 25:start + (index + 1) * 25]
        assert len(block) == 25
        blocks.append({'name': label, 'number': number(block, 5), 'native': block[0],
                       'flags': block[15], 'extended': block[16], 'raw_hex': block.hex()})
    position += physical * 52 + logical * 28
    for _ in range(logical):
        width = number(data, position)
        position += 2 + width

    pairs = []
    complete = False
    for _ in range(257):
        if position + 2 > len(data):
            break
        column = number(data, position)
        if column == 0xffff:
            complete = True
            break
        if position + 10 > len(data) or any(pair['column'] == column for pair in pairs):
            break
        owned = number(data, position + 2, 4)
        candidate = number(data, position + 6, 4)
        pairs.append({'column': column, 'owned': owned, 'candidate': candidate,
                      'logical_offset': position})
        position += 10
        if owned == 0 or candidate == 0:
            break
    return {'complete': complete, 'pairs': pairs, 'columns': blocks, 'pages': pages,
            'definition_sha256': hashlib.sha256(data).hexdigest()}


def footprint(raw, reference):
    row = LONG.frame(raw, reference >> 8, reference & 255)
    assert row['word'] & 0xe000 == 0
    body = raw[row['start']:row['end']]
    bitmap_pages = []
    if body[0] == 1:
        assert (len(body) - 1) % 4 == 0
        for offset in range(1, len(body), 4):
            page = number(body, offset, 4)
            if page:
                assert page < len(raw) // PAGE_BYTES
                assert raw[page * PAGE_BYTES:page * PAGE_BYTES + 4] == b'\x05\x01\x00\x00'
                bitmap_pages.append(page)
    else:
        assert body[0] == 0 and len(body) >= 5
    return {'reference': reference, 'row_start': row['start'], 'row_end': row['end'],
            'container': row['page'], 'bitmaps': bitmap_pages, 'kind': body[0]}


def claims(raw, tables, inventory, maps, indexes):
    result = {}
    for role, metadata in maps.items():
        canonical = role
        if ':Index:' in role:
            table_page, index_name = role.split(':Index:')
            table = next(name for name, value in tables.items()
                         if 'Table:' + str(value['definition']) == table_page)
            canonical = table_page + ':IndexData:' + str(indexes[table][index_name]['physical'])
        item = footprint(raw, metadata['map_page'] << 8 | metadata['map_slot'])
        if canonical in result:
            assert result[canonical] == item
        result[canonical] = item
    for name, declaration in inventory.items():
        owner = tables[name]['definition']
        for pair in declaration['pairs']:
            for suffix, key in (('Owned', 'owned'), ('Hint', 'candidate')):
                reference = pair[key]
                if reference == 0:
                    continue
                canonical = f"Table:{owner}:Column:{pair['column']}:{suffix}"
                item = footprint(raw, reference)
                if canonical in result:
                    assert result[canonical] == item
                result[canonical] = item
    return result


def snapshot(path, stdout, stage):
    raw = path.read_bytes()
    assert 0 < len(raw) <= MAX_FILE_BYTES and len(raw) % PAGE_BYTES == 0
    file_pages = len(raw) // PAGE_BYTES
    tables = {}
    maps = {}
    rows = {SELECTED: {}, PADDING: {}}
    values = {}
    raw_rows = {}
    absent = []
    entries = {}
    indexes = {}
    for line in stdout.read_text().splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'TABLE':
            name = fields[1]
            assert name not in tables
            tables[name] = dict(zip(('definition', 'rows', 'columns_count', 'max_columns'),
                                    map(int, fields[2:]), strict=True))
        elif tag == 'MAP':
            role = fields[1]
            assert role not in maps
            maps[role] = dict(zip(('table_page', 'row_start', 'row_end', 'reported_count'),
                                  map(int, fields[2:]), strict=True), all_members=[])
        elif tag == 'MEMBER':
            maps[fields[1]]['all_members'].append(int(fields[2]))
        elif tag == 'MAP_STATUS':
            absent.append(fields[1:])
        elif tag == 'ROW':
            table, identity = fields[1], int(fields[2])
            assert identity not in rows[table]
            rows[table][identity] = dict(zip(('page', 'slot', 'bytes'), map(int, fields[3:6]), strict=True),
                                         sha256=fields[6])
        elif tag == 'VALUE':
            values[(int(fields[1]), fields[2])] = bytes.fromhex(fields[3])
        elif tag == 'RAWROW':
            raw_rows[int(fields[1])] = bytes.fromhex(fields[2])
        elif tag == 'ENTRY':
            entries.setdefault(fields[1], []).append(int(fields[2]))
        elif tag == 'INDEX':
            indexes.setdefault(fields[1], {})[fields[2]] = dict(
                zip(('physical', 'root', 'entries'), map(int, fields[3:]), strict=True))
        else:
            assert tag in ('COLUMN', 'PROPERTY')

    expected_rows = 2 if stage == 'before' else 1
    expected_padding = 0 if stage == 'after-first' else 700
    assert set(rows[SELECTED]) == set(range(1, expected_rows + 1))
    assert set(rows[PADDING]) == set(range(1, expected_padding + 1))
    assert tables[SELECTED]['rows'] == expected_rows and tables[PADDING]['rows'] == expected_padding
    assert set(entries) == {'PrimaryID', 'IDAsc', 'IDTwin'}
    assert all(order == list(range(1, expected_rows + 1)) for order in entries.values())

    inventory = {name: declarations(raw, table) for name, table in tables.items()}
    for name in (SELECTED, PADDING):
        assert inventory[name]['complete']
    assert [(column['native'], column['flags'], column['extended'])
            for column in inventory[SELECTED]['columns']] == [(4, 3, 0), (12, 2, 0), (11, 2, 0)]

    decoded = {}
    for role, metadata in maps.items():
        metadata['file_members'] = [page for page in metadata['all_members'] if page < file_pages]
        entry = MAP.decode_map(raw, metadata, PAGE_BYTES, file_pages)
        entry['raw_row_hex'] = raw[entry['row_start']:entry['row_end']].hex()
        entry['all_members'] = metadata['all_members']
        decoded[role] = entry

    global_free = set(decoded['GlobalFree']['file_members'])
    bitmap_storage = {}
    for role, entry in decoded.items():
        assert entry['map_page'] not in global_free
        for page in entry['bitmap_pages']:
            body = raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
            assert body[:4] == b'\x05\x01\x00\x00' and page not in global_free
            bitmap_storage.setdefault(page, []).append(role)
    bitmap_proofs = {page: {'roles': roles,
                           'sha256': hashlib.sha256(raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]).hexdigest(),
                           'header_hex': raw[page * PAGE_BYTES:page * PAGE_BYTES + 4].hex(),
                           'allocated': page not in global_free}
                     for page, roles in bitmap_storage.items()}

    longs = {}
    for identity, location in rows[SELECTED].items():
        final, path_frames = LONG.final_frame(raw, location)
        body = raw[final['start']:final['end']]
        assert body == raw_rows[identity]
        fields = {}
        for label, native, variable, column_number in (('Memo', 12, 0, 1), ('OLE', 11, 1, 2)):
            value = LONG.long_value(raw, body, {'variable': variable, 'number': column_number,
                                               'native_type': native})
            expected = (('Ā' if identity == 1 else 'ā') * 8192).encode('utf-16le') if label == 'Memo' else (
                bytes(range(256)) if identity == 1 else bytes(reversed(range(256))))
            assert values[(identity, label)] == expected
            actual = value['logical'].encode('utf-16le') if label == 'Memo' else bytes.fromhex(value['logical']['binary_hex'])
            assert actual == expected
            fields[label] = value
        longs[identity] = {'path': path_frames, 'fields': fields}

    owner = tables[SELECTED]['definition']
    for column in inventory[SELECTED]['columns']:
        if column['native'] not in (11, 12):
            continue
        role = f"Table:{owner}:Column:{column['number']}"
        owned = set(decoded[role + ':Owned']['file_members'])
        candidate = set(decoded[role + ':Hint']['file_members'])
        assert candidate <= owned and owned.isdisjoint(global_free)
        for row in longs.values():
            assert all(chunk['page'] in owned for chunk in row['fields'][column['name']]['chunks'])

    role_claims = claims(raw, tables, inventory, decoded, indexes)
    return {'file': common.pin(path), 'tables': tables, 'declarations': inventory, 'maps': decoded,
            'bitmap_storage': bitmap_proofs, 'indexes': indexes, 'absent_current_holders': absent,
            'role_storage_claims': role_claims, 'rows': rows, 'long_values': longs, 'index_orders': entries}


def unchanged_previous(previous, current):
    for table, rows in previous['rows'].items():
        for identity, row in rows.items():
            assert current['rows'][table][identity] == row
    for identity, row in previous['long_values'].items():
        assert current['long_values'][identity] == row


def validate(freeze_path):
    frozen = json.loads(freeze_path.read_text())
    for expected in frozen['inputs']:
        assert common.pin(expected['path']) == expected
    assert frozen['recipe']['profiles'] == list(PROFILES)
    assert frozen['recipe']['max_fixture_processes'] == MAX_PROCESSES
    assert frozen['recipe']['padding_rows'] == 700
    return frozen


def execute(freeze_path, work):
    frozen = validate(freeze_path)
    directory = common.fresh_work(work)
    capture = common.Capture(directory)
    result = {'scope': 'public-builder observations only; no native candidate',
              'freeze': common.pin(freeze_path), 'profiles': {}, 'commands': []}

    def command(argv, label):
        assert len(capture.commands) < MAX_PROCESSES
        entry = capture.run(argv, label)
        assert entry['status'] == 0 and not entry['timed_out'], label
        return common.path(entry['stdout']['path'])

    try:
        classes = directory / 'classes'
        classes.mkdir()
        jar = common.path(frozen['recipe']['jar'])
        helper = DIRECTORY / 'IndirectLongValueProbe.java'
        command(['javac', '-J-Xmx512m', '-cp', str(jar), '-d', str(classes), str(helper)], 'compile')
        result['compiled_classes'] = [common.pin(path) for path in sorted(classes.rglob('*.class'))]
        java = ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8', '-cp',
                os.pathsep.join((str(jar), str(classes))), 'IndirectLongValueProbe']
        originals = directory / 'originals'
        originals.mkdir()
        for profile in PROFILES:
            working = originals / (profile + '-working.accdb')
            result['profiles'][profile] = stages = {}
            for stage, mode in zip(STAGES, ('create-first', 'pad', 'second'), strict=True):
                argv = java + [mode, str(working)] + ([profile] if mode == 'create-first' else [])
                command(argv, profile + '-' + mode)
                before_copy = common.pin(working)
                target = originals / (profile + '-' + stage + '.accdb')
                shutil.copyfile(working, target)
                assert common.pin(target)['sha256'] == before_copy['sha256']
                before_getter = common.pin(target)
                output = command(java + ['inspect', str(target)], profile + '-' + stage + '-inspect')
                assert common.pin(target) == before_getter
                stages[stage] = snapshot(target, output, stage)
                if len(stages) > 1:
                    previous_stage = STAGES[STAGES.index(stage) - 1]
                    unchanged_previous(stages[previous_stage], stages[stage])
                assert common.pin(working) == before_copy
                save(directory / 'partial.json', result)

            final = stages['before']
            owner = final['tables'][SELECTED]['definition']
            memo_number = next(column['number'] for column in final['declarations'][SELECTED]['columns']
                               if column['name'] == 'Memo')
            role = f'Table:{owner}:Column:{memo_number}'
            owned = final['maps'][role + ':Owned']
            candidate = final['maps'][role + ':Hint']
            members = owned['file_members']
            stages['observed_geometry'] = {
                'memo_owned_type': owned['payload_type'],
                'memo_candidate_type': candidate['payload_type'],
                'memo_owned_span': max(members) - min(members) + 1,
                'memo_owned_bitmap_pages': owned['bitmap_pages'],
                'memo_candidate_bitmap_pages': candidate['bitmap_pages'],
                'owned_indirect_hypothesis_met': owned['payload_type'] == 1,
                'candidate_indirect_observed': candidate['payload_type'] == 1,
                'no_recipe_adaptation': True,
            }

        result['completed'] = True
    except Exception as error:
        result['completed'] = False
        result['error'] = {'type': type(error).__name__, 'message': str(error),
                           'traceback': traceback.format_exc()}
    finally:
        result['commands'] = capture.commands
        result['immutable_inputs_unchanged'] = all(common.pin(entry['path']) == entry for entry in frozen['inputs'])
        assert common.pin(freeze_path) == result['freeze']
        save(directory / 'report.json', result)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--work')
    arguments = parser.parse_args()
    if arguments.execute:
        if arguments.work is None:
            parser.error('fresh --work required')
        result = execute(arguments.freeze, arguments.work)
        print(json.dumps({'completed': result['completed'], 'commands': len(result['commands'])}))
        raise SystemExit(0 if result['completed'] else 1)
    validate(arguments.freeze)
    print('Static input validation only; no processes started')


if __name__ == '__main__':
    main()
