"""Public transition from source/request facts; no expected-image inputs."""
import hashlib

from ace16_insert_extended_datetime import source

PAGE = 4096


def compare(before, path, getter, requests):
    raw = path.read_bytes()
    original = before['raw']
    assert len(raw) == len(original)
    tables, descriptions, properties, absent = source.parse(getter)
    assert set(tables) == set(before['tables'])
    assert properties == before['properties'] and absent == before['absent']
    maps = {}
    for role, description in descriptions.items():
        members = description['all_members']
        assert members == sorted(set(members))
        description['file_members'] = [page for page in members if page < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    assert maps == before['maps']
    declarations = {name: source.native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(value['complete'] for value in declarations.values())
    claims = source.native.declarations.claims(raw, tables, declarations, maps,
                                             {name: table['indexes'] for name, table in tables.items()})
    assert claims == before['map_claims']
    table, old_table = tables['Items'], before['table']
    for name, actual in tables.items():
        old = before['tables'][name]
        for key in ('definition', 'columns_count', 'max_columns', 'columns'):
            assert actual[key] == old[key]
        if name != 'Items':
            assert actual == old
    assert table['count'] == len(table['rows']) == old_table['count'] + len(requests)
    added = {identity: dict(ID=identity, Group=group, Stamp=stamp)
             for identity, group, stamp in requests}
    assert len(added) == len(requests) == 6
    assert not set(added).intersection(old_table['rows'])
    assert table['values'] == old_table['values'] | added
    assert set(table['rows']) == set(old_table['rows']) | set(added)
    source.bind_definition(raw, table, claims, ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias'])
    source.ordinary_rows(raw, table, maps)
    for identity, row in old_table['rows'].items():
        assert table['rows'][identity] == row
        frame, chain = source.native.long_wire.final_frame(raw, row)
        assert raw[frame['start']:frame['end']] == row['raw']

    allowed = bytearray(len(original))

    def allow(start, end):
        assert 0 <= start <= end <= len(original)
        allowed[start:end] = bytes([255]) * (end - start)

    count_at = table['definition'] * PAGE + 16
    assert source.native.number(raw, count_at, 4) == table['count']
    allow(count_at, count_at + 4)
    frames = []
    null_payloads = {}
    for identity, group, stamp in requests:
        row = table['rows'][identity]
        frame, chain = source.native.long_wire.final_frame(raw, row)
        assert len(chain) == 1 and frame['word'] & 0xe000 == 0
        body = raw[frame['start']:frame['end']]
        assert len(body) == 53 and source.native.number(body, 0) == 3
        assert body[-1] == 1 | (group is not None) * 2 | (stamp is not None) * 4
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        if group is not None:
            assert int.from_bytes(body[6:10], 'little', signed=True) == group
        if stamp is None:
            # A public NULL's inactive fixed cell is observed, not forced to
            # the engineering constructor's explicit all-zero byte policy.
            null_payloads[str(identity)] = body[10:52].hex()
        else:
            assert source.timestamp(body[10:52]) == stamp
        base = frame['page'] * PAGE
        old_count = source.native.number(original, base + 12)
        assert frame['slot'] >= old_count
        allow(base + 2, base + 4)
        allow(base + 12, base + 14)
        allow(base + 14 + frame['slot'] * 2, base + 16 + frame['slot'] * 2)
        allow(frame['start'], frame['end'])
        frames.append(dict(identity=identity, reference=row['page'] << 8 | row['slot'],
                           page=frame['page'], slot=frame['slot'], bytes=len(body)))
    assert len({item['reference'] for item in frames}) == 6
    index_scratch = {}
    histories = {}
    for physical, column, names in [(0, 'ID', ('PrimaryID', 'PrimaryAlias')),
                                     (1, 'Group', ('GroupOrder', 'GroupAlias'))]:
        old = old_table['indexes'][names[0]]
        selected = table['indexes'][names[0]]
        for name in names:
            index = table['indexes'][name]
            for key in ('physical', 'root', 'history_offset', 'flags', 'primary', 'columns'):
                assert index[key] == old_table['indexes'][name][key]
            assert index['entries'] == table['count']
        nodes, leaves, entries = source.native.index_wire.tree(raw, selected['root'], table['definition'])
        assert set(nodes) == set(before['trees'][physical]['nodes'])
        assert len(nodes) == len(leaves) == 1
        expected = sorted(source.native.index_wire.key(table['values'][identity][column],
                           row['page'] << 8 | row['slot']) for identity, row in table['rows'].items())
        assert entries == expected
        references = [int.from_bytes(entry[-4:], 'big') for entry in entries]
        for name in names:
            assert [page << 8 | slot for identity, page, slot in table['indexes'][name]['rows']] == references
        old_families = {row[column] for row in old_table['values'].values()}
        new_families = {row[column] for row in added.values()} - old_families
        history = old['history'] + len(new_families)
        assert all(table['indexes'][name]['history'] == history for name in names)
        at = before['trees'][physical]['history_at']
        assert source.native.number(raw, at, 4) == history
        allow(at, at + 4)
        # The public serializer owns this verified index frame and its scratch;
        # its byte policy never supplies the engineering expected-image bytes.
        start = selected['root'] * PAGE
        allow(start, start + PAGE)
        active_end = start + 480 + nodes[selected['root']]['active']
        index_scratch[str(physical)] = sum(left != right for left, right in
                                          zip(original[active_end:start + PAGE], raw[active_end:start + PAGE]))
        histories[str(physical)] = history
    for position, (old, actual) in enumerate(zip(original, raw)):
        assert old == actual or allowed[position], ('Protected source prefix', position, old, actual)
    census = source.native.census.prove(raw, tables, declarations, maps, claims, {}, source.native.long_wire)
    return dict(after_sha256=hashlib.sha256(raw).hexdigest(), after_bytes=len(raw),
                table_rows=table['count'], histories=histories, new_rows=frames,
                new_NULL_stamp_payloads=null_payloads, index_unused_scratch_changed_bytes=index_scratch,
                allocations=0, map_changes=0, old_rows_preserved=len(old_table['rows']),
                catalog_census=census)
