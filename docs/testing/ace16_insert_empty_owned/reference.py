"""Public transition from immutable SOURCE/request facts; no engineering images."""
import hashlib

from ace16_insert_empty_owned import source

PAGE = 4096


def stable_map(value):
    # The decoder reports locations for every now-in-file bit, including newly
    # appended zero bits in otherwise unchanged maps. Those lookup inventories
    # grow with EOF; raw map bytes and semantic counts/members stay protected.
    return {name: item for name, item in value.items() if name != 'bit_locations'}


def compare(before, path, getter, requests):
    raw = path.read_bytes()
    original = before['raw']
    assert len(original) < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    tables, descriptions, properties, absent = source.native.parse(getter.read_text())
    assert set(tables) == set(before['tables'])
    assert properties == before['properties'] and absent == before['absent']
    maps = {}
    for role, description in descriptions.items():
        members = description['all_members']
        assert members == sorted(set(members))
        description['file_members'] = [page for page in members if page < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    assert set(maps) == set(before['maps'])
    table, old = tables['Items'], before['table']
    assert old['count'] == 0 and old['rows'] == old['values'] == {}
    owner = 'Table:' + str(table['definition'])
    wanted = {identity: {'ID': identity, 'Bytes': bytes.fromhex(payload)}
              for identity, payload in requests}
    assert len(wanted) == len(requests)
    assert table['count'] == len(table['rows']) == len(wanted)
    assert table['values'] == wanted and set(table['rows']) == set(wanted)
    for name, actual in tables.items():
        previous = before['tables'][name]
        for field in ('definition', 'columns_count', 'max_columns', 'columns'):
            assert actual[field] == previous[field]
        if name != 'Items':
            assert stable_map(actual) == stable_map(previous)
    source.scalar_rows(raw, table, maps)
    data_pages = set(maps[owner + ':Owned']['file_members'])
    assert before['maps'][owner + ':Owned']['file_members'] == []
    assert data_pages and min(data_pages) >= len(original) // PAGE
    assert set(maps[owner + ':Hint']['file_members']) <= data_pages
    owned = maps[owner + ':Owned']
    previous_owned = before['maps'][owner + ':Owned']
    claim = before['map_claims'][owner + ':Owned']
    start, end = claim['row_start'], claim['row_end']
    assert end - start == 69 and original[start] == raw[start] == 1
    assert source.native.number(original, start + 1, 4) == 0
    bitmap = source.native.number(raw, start + 1, 4)
    assert len(original) // PAGE <= bitmap < len(raw) // PAGE
    assert raw[start + 5:end] == original[start + 5:end]
    assert owned['bitmap_pages'] == [bitmap] + previous_owned['bitmap_pages']
    assert bitmap not in data_pages and bitmap not in maps[owner + ':Hint']['file_members']
    expected_bitmap = bytearray(PAGE)
    expected_bitmap[:4] = b'\x05\x01\0\0'
    for member in data_pages:
        assert member < (PAGE - 4) * 8
        expected_bitmap[4 + member // 8] |= 1 << (member % 8)
    assert raw[bitmap * PAGE:(bitmap + 1) * PAGE] == expected_bitmap
    assert owned['outside_file_set_bit_count'] == 0
    assert set(range(len(original) // PAGE, len(raw) // PAGE)) == data_pages | {bitmap}
    assert bitmap == min(data_pages) + 1

    allowed = bytearray(len(original))

    def allow(begin, finish):
        assert 0 <= begin <= finish <= len(original)
        allowed[begin:finish] = bytes([255]) * (finish - begin)

    allow(start + 1, start + 5)
    count_at = table['definition'] * PAGE + 16
    assert source.native.number(raw, count_at, 4) == len(requests)
    allow(count_at, count_at + 4)
    frames = []
    padding = {}
    for identity, payload in requests:
        row = table['rows'][identity]
        frame, chain = source.native.long_wire.final_frame(raw, row)
        assert len(chain) == 1 and frame['word'] & 0xe000 == 0
        assert (frame['page'], frame['slot']) == (row['page'], row['slot'])
        assert (row['page'], row['slot']) == (row['target_page'], row['target_slot'])
        body = raw[frame['start']:frame['end']]
        assert body == row['raw'] and source.native.number(body, 0) == 2
        assert len(body) >= 13 and body[-1] == 3
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        mask = len(body) - 1
        assert source.native.number(body, mask - 2) == 1
        begin = source.native.number(body, mask - 4)
        finish = source.native.number(body, mask - 6)
        assert 6 <= begin <= finish <= mask - 6
        assert body[begin:finish] == bytes.fromhex(payload)
        padding[str(identity)] = {'before_variable': body[6:begin].hex(),
                                  'after_variable': body[finish:mask - 6].hex()}
        frames.append(dict(identity=identity, page=row['page'], slot=row['slot'],
                           reference=row['page'] << 8 | row['slot'], bytes=len(body)))
    assert len({frame['reference'] for frame in frames}) == len(requests)
    assert {frame['page'] for frame in frames} == data_pages
    assert maps[owner + ':Hint']['file_members'] == [frames[-1]['page']]
    assert [frame['reference'] for frame in frames] == sorted(frame['reference'] for frame in frames)
    assert list(table['indexes']) == ['PrimaryID', 'PrimaryAlias']
    selected = table['indexes']['PrimaryID']
    old_selected = old['indexes']['PrimaryID']
    expected_keys = []
    for frame in frames:
        key = b'\x7f' + ((frame['identity'] & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
        expected_keys.append(key + frame['reference'].to_bytes(4, 'big'))
    expected_keys.sort()
    for name in ('PrimaryID', 'PrimaryAlias'):
        index = table['indexes'][name]
        previous = old['indexes'][name]
        for field in ('physical', 'root', 'history_offset', 'flags', 'primary', 'columns'):
            assert index[field] == previous[field]
        assert index['entries'] == index['history'] == len(requests)
        references = [int.from_bytes(entry[-4:], 'big') for entry in expected_keys]
        assert [page << 8 | slot for identity, page, slot in index['rows']] == references
        for identity, page, slot in index['rows']:
            assert (page, slot) == (table['rows'][identity]['page'], table['rows'][identity]['slot'])
    root = selected['root']
    nodes, leaves, entries = source.native.index_wire.tree(raw, root, table['definition'])
    assert list(nodes) == leaves == [root] and entries == expected_keys
    assert stable_map(maps[owner + ':IndexData:0']) == stable_map(before['maps'][owner + ':IndexData:0'])
    history_at = table['definition'] * PAGE + old_selected['history_offset']
    assert old_selected['history'] == 0 and source.native.number(raw, history_at, 4) == len(requests)
    allow(history_at, history_at + 4)
    # Complete decoded framing and entries certify the selected root. Public
    # inactive scratch is observed separately, never copied into a constructor.
    allow(root * PAGE, (root + 1) * PAGE)
    active_end = root * PAGE + 480 + nodes[root]['active']
    scratch = sum(left != right for left, right in
                  zip(original[active_end:(root + 1) * PAGE], raw[active_end:(root + 1) * PAGE]))
    suffix = set(range(len(original) // PAGE, len(raw) // PAGE))
    for role, actual in maps.items():
        previous = before['maps'][role]
        for field in ('map_page', 'map_slot', 'row_start', 'row_end'):
            assert actual[field] == previous[field]
        if role == owner + ':Owned':
            continue
        if role == owner + ':Hint':
            assert actual['payload_type'] == 0 and actual['outside_file_set_bit_count'] == 0
            assert actual['row_end'] - actual['row_start'] == 69
            allow(actual['row_start'], actual['row_end'])
        elif role == 'GlobalFree':
            assert actual['payload_type'] == previous['payload_type']
            assert actual['bitmap_pages'] == previous['bitmap_pages']
            assert actual['file_members'] == previous['file_members']
            assert actual['complete_set_bit_count'] == previous['complete_set_bit_count'] - len(suffix)
            for member in suffix:
                offset, bit = source.native.map_cell(original, before['map_claims'][role], member)
                assert original[offset] & bit and not raw[offset] & bit
                allowed[offset] |= bit
        else:
            assert stable_map(actual) == stable_map(previous)
    for position, (before_byte, after_byte) in enumerate(zip(original, raw)):
        assert (before_byte ^ after_byte) & ~allowed[position] == 0, ('Protected source prefix', position)
    declarations = {name: source.native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(value['complete'] for value in declarations.values())
    claims = source.native.declarations.claims(raw, tables, declarations, maps,
                                             {name: table['indexes'] for name, table in tables.items()})
    source.require_exclusive(claims, ['GlobalFree', owner + ':Owned', owner + ':Hint', owner + ':IndexData:0'])
    census = source.native.census.prove(raw, tables, declarations, maps, claims, {}, source.native.long_wire)
    assert set(maps['GlobalFree']['file_members']).isdisjoint(map(int, census['protected_pages']))
    return dict(
        rows=len(requests), source_pages=len(original) // PAGE, final_pages=len(raw) // PAGE,
        sha256=hashlib.sha256(raw).hexdigest(), data_pages=sorted(data_pages), owned_bitmap=bitmap,
        owned_reference=claim['reference'], candidate_members=maps[owner + ':Hint']['file_members'],
        candidate_row_hex=raw[maps[owner + ':Hint']['row_start']:maps[owner + ':Hint']['row_end']].hex(),
        higher_pointer_bytes_preserved=True, rows_and_references=frames,
        physical_history=len(requests), root=root, index_prefix=nodes[root]['prefix'],
        index_active=nodes[root]['active'], index_scratch_differences=scratch,
        public_record_padding=padding, catalog_long_tables=census['long_tables'],
    )
