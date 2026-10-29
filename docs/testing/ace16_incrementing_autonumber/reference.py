"""Public generated INSERT facts from immutable SOURCE/companions, never model bytes."""
from pathlib import Path
import hashlib

from ace16_incrementing_autonumber import source

PAGE = 4096


def data_rows(raw, table, maps):
    live, physical = {}, {}
    for number in maps['Table:' + str(table['definition']) + ':Owned']['file_members']:
        base = number * PAGE
        page = raw[base:base + PAGE]
        assert page[:2] == b'\x01\x01' and source.native.number(page, 4, 4) == table['definition']
        count = source.native.number(page, 12)
        assert count <= 255
        end = PAGE
        for slot in range(count):
            word = source.native.number(page, 14 + slot * 2)
            begin = word & 0x1fff
            flags = word & 0xe000
            assert flags in (0, 0x8000, 0xc000)
            assert 14 + count * 2 <= begin < end <= PAGE
            body = page[begin:end]
            assert len(body) == 15 and body[:2] == b'\x03\x00' and body[-1] in (3, 7)
            reference = number << 8 | slot
            entry = dict(reference=reference, page=number, slot=slot, word=word,
                         start=base + begin, end=base + end, body=body)
            assert reference not in physical
            physical[reference] = entry
            if not flags:
                values = [int.from_bytes(body[2:6], 'little', signed=True),
                          int.from_bytes(body[6:10], 'little', signed=True),
                          None if body[-1] & 4 == 0 else int.from_bytes(body[10:14], 'little', signed=True)]
                entry['values'] = values
                live[reference] = entry
            end = begin
        assert source.native.number(page, 2) == end - 14 - 2 * count
    assert len(live) == table['count']
    return live, physical


def encoded(value, reference):
    key = b'\x00' if value is None else b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return key + reference.to_bytes(4, 'big')


def compare(before, case, after_path, getter_path, append_stdout=None):
    original = before['raw']
    raw = Path(after_path).read_bytes()
    assert len(raw) % PAGE == 0 and len(original) <= len(raw) <= 8388608
    generated = [before['counter_last'] + position + 1 for position in range(len(case['requests']))]
    assert all(-(1 << 31) <= identity < 1 << 31 for identity in generated)
    # Public reference callers supply the real public returned-values stream.
    # Native fresh-reader callers check their separate native generation result
    # before invoking this same raw typed transition proof without that stream.
    if append_stdout is not None:
        output = Path(append_stdout).read_text().splitlines()
        assert output == ['GENERATED\t' + str(position) + '\t' + str(identity)
                          for position, identity in enumerate(generated)]
    tables, metadata, properties, absent, counters = source.parse(Path(getter_path).read_text())
    assert set(tables) == set(before['tables']) and set(counters) == set(tables)
    maps = {}
    for role, item in metadata.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [page for page in item['all_members'] if page < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    assert set(maps) == set(before['maps'])
    declarations = {name: source.native.declarations.declarations(raw, table) for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = source.native.declarations.claims(raw, tables, declarations, maps,
                                             {name: table['indexes'] for name, table in tables.items()})
    assert set(claims) == set(before['claims'])
    before_tables, _, before_properties, before_absent, _ = source.parse(
        Path(case['before_getter']).read_text())
    assert properties == before_properties and absent == before_absent
    table = tables['Items']
    owner = table['definition']
    prefix = 'Table:' + str(owner)
    assert owner == before['tables']['Items']['definition']
    assert table['columns'] == before['tables']['Items']['columns']
    assert table['count'] == len(before['live_rows']) + len(generated)
    for name in tables:
        assert tables[name]['definition'] == before_tables[name]['definition']
        assert declarations[name]['columns'] == before['declarations'][name]['columns']
        assert declarations[name]['pairs'] == before['declarations'][name]['pairs']
        wanted = [generated[-1], before['complex_last']] if name == 'Items' else [
            int.from_bytes(original[tables[name]['definition'] * PAGE + 20:tables[name]['definition'] * PAGE + 24], 'little', signed=True),
            int.from_bytes(original[tables[name]['definition'] * PAGE + 28:tables[name]['definition'] * PAGE + 32], 'little', signed=True)]
        body, _ = source.native.declarations.definition(raw, tables[name]['definition'])
        assert counters[name] == wanted
        assert [int.from_bytes(body[20:24], 'little', signed=True),
                int.from_bytes(body[28:32], 'little', signed=True)] == wanted
    logical, _ = source.records.index_declarations(raw, table, maps)
    assert logical == before['logical']
    live, physical = data_rows(raw, table, maps)
    assert len(table['rows']) == len(table['values']) == len(live)
    by_id = {}
    for reference, row in live.items():
        earlier, identity, score = row['values']
        assert identity not in by_id
        by_id[identity] = row
        reported = table['rows'][identity]
        assert reference == (reported['page'] << 8 | reported['slot'])
        assert reported['page'] == reported['target_page'] and reported['slot'] == reported['target_slot']
        assert reported['raw'] == row['body']
        assert table['values'][identity] == dict(Earlier=earlier, ID=identity, Score=score)
    old_ids = {values[1] for values in before['live_rows'].values()}
    assert not old_ids & set(generated) and set(by_id) == old_ids | set(generated)
    for reference, values in before['live_rows'].items():
        assert reference in live and live[reference]['values'] == values
        old = before['tables']['Items']['rows'][values[1]]['raw']
        assert live[reference]['body'] == old
    new_rows = [by_id[identity] for identity in generated]
    for row, identity, (earlier, score) in zip(new_rows, generated, case['requests'], strict=True):
        assert row['values'] == [earlier, identity, score]
    original_dead = {item['reference']: item for item in before['deleted_rows']}
    new_refs = {item['reference'] for item in new_rows}
    assert set(physical) - set(live) <= set(original_dead)
    allowed = bytearray(len(original))

    def allow(start, end):
        assert 0 <= start <= end <= len(original)
        allowed[start:end] = bytes([255]) * (end - start)

    old_data = set(before['maps'][prefix + ':Owned']['file_members'])
    new_data = set(maps[prefix + ':Owned']['file_members']) - old_data
    assert old_data <= set(maps[prefix + ':Owned']['file_members'])
    assert all(page >= len(original) // PAGE for page in new_data)
    assert set(maps[prefix + ':Hint']['file_members']) <= set(maps[prefix + ':Owned']['file_members'])
    for row in new_rows:
        if row['page'] < len(original) // PAGE:
            assert row['page'] in old_data
            allow(row['page'] * PAGE + 2, row['page'] * PAGE + 4)
            allow(row['page'] * PAGE + 12, row['page'] * PAGE + 14)
            allow(row['page'] * PAGE + 14 + row['slot'] * 2, row['page'] * PAGE + 16 + row['slot'] * 2)
            allow(row['start'], row['end'])
    reused = []
    for reference, old in original_dead.items():
        page, slot = reference >> 8, reference & 255
        directory = page * PAGE + 14 + slot * 2
        begin = source.native.number(original, directory) & 0x1fff
        end = PAGE if slot == 0 else source.native.number(original, directory - 2) & 0x1fff
        if reference in new_refs:
            reused.append(reference)
        else:
            assert raw[directory:directory + 2] == original[directory:directory + 2]
            assert raw[page * PAGE + begin:page * PAGE + end] == bytes.fromhex(old['raw'])
    suffix_roles = {page: 'DATA' for page in new_data}
    index_facts = []
    for physical_number, name, column in [(0, 'PrimaryID', 1), (1, 'ScoreOrder', 2)]:
        index = table['indexes'][name]
        old_index = before['tables']['Items']['indexes'][name]
        assert (index['physical'], index['root'], index['flags'], index['columns']) == (
            old_index['physical'], old_index['root'], old_index['flags'], old_index['columns'])
        nodes, leaves, stream = source.native.index_wire.tree(raw, index['root'], owner)
        assert stream == sorted(encoded(row['values'][column], reference) for reference, row in live.items())
        old_families = {values[column] for values in before['live_rows'].values()}
        new_families = {row['values'][column] for row in live.values()}
        assert index['history'] == old_index['history'] + len(new_families - old_families)
        assert index['entries'] == len(live)
        for alias, candidate in table['indexes'].items():
            if candidate['physical'] == physical_number:
                expected_alias = before['tables']['Items']['indexes'][alias]
                assert candidate['root'] == index['root'] and candidate['flags'] == index['flags']
                assert candidate['history'] == index['history'] and candidate['columns'] == expected_alias['columns']
                assert candidate['primary'] == expected_alias['primary']
                assert candidate['rows'] == [(live[int.from_bytes(entry[-4:], 'big')]['values'][1],
                                             int.from_bytes(entry[-4:], 'big') >> 8,
                                             int.from_bytes(entry[-4:], 'big') & 255) for entry in stream]
        members = set(maps[prefix + ':IndexData:' + str(physical_number)]['file_members'])
        assert members == set(nodes)
        if case['state'] == 'pressure':
            assert len(nodes) == 3 and len(leaves) == 2
        else:
            assert list(nodes) == leaves == [index['root']]
        for page in members - {index['root']}:
            assert page >= len(original) // PAGE and page not in suffix_roles
            suffix_roles[page] = 'INDEX:' + str(physical_number)
        # Public index framing and inactive scratch are observations. Complete
        # key/ROW streams, owners, heights and alias declarations were checked.
        root = index['root'] * PAGE
        allow(root, root + PAGE)
        before_page = original[root:root + PAGE]
        after_page = raw[root:root + PAGE]
        active_end = 480 + 3616 - source.native.number(after_page, 2)
        index_facts.append(dict(physical=physical_number, root=index['root'],
                                before_prefix=source.native.number(before_page, 24),
                                after_prefix=source.native.number(after_page, 24),
                                nodes=sorted(nodes), leaves=leaves,
                                new_unused_byte_changes=sum(a != b for a, b in zip(before_page[active_end:], after_page[active_end:])),
                                history_before=old_index['history'], history_after=index['history']))
    for role, before_map in before['maps'].items():
        after_map = maps[role]
        claim, old_claim = claims[role], before['claims'][role]
        assert (claim['reference'], claim['row_start'], claim['row_end']) == (
            old_claim['reference'], old_claim['row_start'], old_claim['row_end'])
        if role == prefix + ':Owned':
            new_bitmaps = set(after_map['bitmap_pages']) - set(before_map['bitmap_pages'])
            assert set(before_map['bitmap_pages']) <= set(after_map['bitmap_pages'])
            for page in new_bitmaps:
                assert page >= len(original) // PAGE and page not in suffix_roles
                suffix_roles[page] = 'ORDINARY_OWNED_BITMAP'
            if new_bitmaps:
                assert case['state'] == 'fresh' and len(new_bitmaps) == 1
                assert not before_map['file_members'] and not before_map['bitmap_pages']
                start, end = claim['row_start'], claim['row_end']
                assert raw[start] == original[start] == 1
                assert raw[start + 5:end] == original[start + 5:end]
                assert source.native.number(raw, start + 1, 4) == next(iter(new_bitmaps))
                allow(start + 1, start + 5)
        elif role not in (prefix + ':Hint', prefix + ':IndexData:0', prefix + ':IndexData:1', 'GlobalFree'):
            assert after_map['file_members'] == before_map['file_members']
            assert after_map['bitmap_pages'] == before_map['bitmap_pages']
            assert after_map['complete_set_bit_count'] == before_map['complete_set_bit_count']
        assert after_map['payload_type'] == before_map['payload_type']
        if role == 'GlobalFree':
            assert after_map['file_members'] == before_map['file_members']
        for page in set(after_map['file_members']) ^ set(before_map['file_members']):
            cell = source.native.represented_cell(raw, claim, page)
            assert cell is not None
            offset, mask = cell
            if offset < len(original):
                allowed[offset] |= mask
    suffix = set(range(len(original) // PAGE, len(raw) // PAGE))
    assert set(suffix_roles) == suffix
    assert len(suffix) == (2 if case['state'] == 'fresh' else 4 if case['state'] == 'pressure' else 0)
    global_claim = claims['GlobalFree']
    for page in suffix:
        offset, mask = source.native.map_cell(raw, global_claim, page)
        assert original[offset] & mask and not raw[offset] & mask
        allowed[offset] |= mask
    assert maps['GlobalFree']['complete_set_bit_count'] == before['maps']['GlobalFree']['complete_set_bit_count'] - len(suffix)
    allow(owner * PAGE + 16, owner * PAGE + 24)
    for index in table['indexes'].values():
        history = owner * PAGE + index['history_offset']
        allow(history, history + 4)
    for offset, (old, new) in enumerate(zip(original, raw)):
        assert (old ^ new) & ~allowed[offset] == 0, ('Protected public source byte', offset)
    source.records.catalog_long_values(raw, tables, declarations, maps, claims)
    return dict(sha256=hashlib.sha256(raw).hexdigest(), bytes=len(raw), generated_values=generated,
                counter_before=before['counter_last'], counter_after=generated[-1],
                source_pages=len(original) // PAGE, final_pages=len(raw) // PAGE,
                inserted_rows=len(generated), original_live_rows_preserved=True,
                reused_deleted_references=reused,
                new_rows=[dict(identity=row['values'][1], reference=row['reference'],
                               record_hex=row['body'].hex(), absent_score_payload=row['body'][10:14].hex()
                               if row['values'][2] is None else None) for row in new_rows],
                suffix_roles=suffix_roles, indexes=index_facts,
                mode='public default-writer observation; stored mode unclassified')
