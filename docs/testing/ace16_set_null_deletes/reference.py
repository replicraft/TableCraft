"""Public DELETE SET NULL transition from immutable original values and seeds."""
import hashlib

from ace16_set_null_deletes import source

PAGE = 4096
DIRECTORY = 14
ENTRY_START = 480
BITMAP_START = 27


def transitions(before, requests):
    tables = before['tables']
    deleted = {name: set() for name in tables}
    pending = list(map(tuple, requests))
    assert pending == [('Parents', 1)]
    relations = before['relationships']
    while pending:
        name, identity = pending.pop()
        if identity in deleted[name]:
            continue
        assert identity in tables[name]['values']
        deleted[name].add(identity)
        for relation in relations:
            if relation['szReferencedObject'] != name or relation['grbit'] != 0x1000:
                continue
            value = tables[name]['values'][identity][relation['szReferencedColumn']]
            child = relation['szObject']
            for child_id, values in tables[child]['values'].items():
                if values[relation['szColumn']] is not None and values[relation['szColumn']] == value:
                    pending.append((child, child_id))
    nulls = set()
    for relation in relations:
        if relation['grbit'] != 0x2000:
            continue
        parent, child = relation['szReferencedObject'], relation['szObject']
        values = {tables[parent]['values'][identity][relation['szReferencedColumn']]
                  for identity in deleted[parent]}
        for identity, row in tables[child]['values'].items():
            column = relation['szColumn']
            if identity not in deleted[child] and row[column] is not None and row[column] in values:
                nulls.add((child, identity, column))
    return deleted, nulls


def compare(before, original_getter, path, getter, requests):
    original = before['raw']
    raw = path.read_bytes()
    assert len(raw) == len(original) <= 8388608
    old_tables, old_metadata, old_properties, old_absent, old_relations = source.parse(original_getter.read_text())
    tables, descriptions, properties, absent, relations = source.parse(getter.read_text())
    assert old_tables == before['tables']
    assert set(tables) == set(old_tables)
    assert properties == old_properties and absent == old_absent and relations == old_relations
    maps = {}
    for role, description in descriptions.items():
        assert description['all_members'] == sorted(set(description['all_members']))
        description['file_members'] = [page for page in description['all_members'] if page < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    assert maps == before['maps']
    deleted, nulls = transitions(before, requests)
    touched = {name for name, rows in deleted.items() if rows}
    touched.update(name for name, identity, column in nulls)
    allowed = bytearray(len(raw))

    def allow(start, end):
        assert 0 <= start <= end <= len(raw)
        allowed[start:end] = bytes([1]) * (end - start)

    null_payload_differences = []
    index_frames = []
    for name, old in old_tables.items():
        table = tables[name]
        for field in ('definition', 'columns_count', 'max_columns', 'columns'):
            assert table[field] == old[field]
        wanted = {identity: dict(values) for identity, values in old['values'].items()
                  if identity not in deleted[name]}
        for table_name, identity, column in nulls:
            if table_name == name:
                wanted[identity][column] = None
        assert table['values'] == wanted
        assert table['count'] == old['count'] - len(deleted[name])
        assert set(table['rows']) == set(old['rows']) - deleted[name]
        if name not in touched:
            assert table == old
            continue
        count_at = old['definition'] * PAGE + 16
        assert source.native.number(raw, count_at, 4) == table['count']
        if deleted[name]:
            allow(count_at, count_at + 4)
        records = source.live_records(raw, table, maps)
        assert {root for root, body in records} == {row['page'] << 8 | row['slot'] for row in table['rows'].values()}
        for identity, previous in old['rows'].items():
            directory = previous['page'] * PAGE + DIRECTORY + previous['slot'] * 2
            word = source.native.number(original, directory)
            assert word & 0xe000 == 0
            start = previous['page'] * PAGE + word
            body = original[start:start + len(previous['raw'])]
            assert body == previous['raw']
            if identity in deleted[name]:
                assert source.native.number(raw, directory) == word | 0xc000
                assert raw[start:start + len(body)] == body
                allow(directory, directory + 2)
                continue
            current = table['rows'][identity]
            for field in ('page', 'slot', 'target_page', 'target_slot'):
                assert current[field] == previous[field]
            assert len(current['raw']) == len(body)
            expected_mask = body[-1]
            for table_name, target, column in nulls:
                if table_name != name or target != identity:
                    continue
                number = old['columns'][column]['number']
                assert number < 8 and len(old['columns']) < 8
                expected_mask &= ~(1 << number)
                declaration = next(item for item in before['declarations'][name]['columns']
                                   if item['number'] == number)
                header = bytes.fromhex(declaration['raw_hex'])
                offset = 2 + source.native.number(header, 21)
                assert declaration['native'] == 4 and declaration['flags'] == 3
                # Public NULL payload is opaque; the engineering model preserves the old bytes.
                null_payload_differences.append(dict(table=name, identity=identity, column=column,
                    before=body[offset:offset + 4].hex(), after=current['raw'][offset:offset + 4].hex()))
                allow(start + offset, start + offset + 4)
                allow(start + len(body) - 1, start + len(body))
            assert current['raw'][-1] == expected_mask
            values = {item['name']:source.scalar_value(current['raw'], item)
                      for item in before['declarations'][name]['columns']}
            assert values == wanted[identity]
        assert set(table['indexes']) == set(old['indexes'])
        handled = set()
        for index_name, old_index in old['indexes'].items():
            index = table['indexes'][index_name]
            for field in ('physical', 'root', 'history', 'history_offset', 'flags', 'primary', 'columns'):
                assert index[field] == old_index[field]
            field = index['columns'][0][0]
            pairs = []
            for identity, values in wanted.items():
                value = values[field]
                if value is None and index['flags'] & 2:
                    continue
                row = table['rows'][identity]
                pairs.append((source.numeric_key(value), row['page'] << 8 | row['slot'], identity))
            pairs.sort()
            assert index['rows'] == [(identity, root >> 8, root & 255) for key, root, identity in pairs]
            assert index['entries'] == len(pairs)
            nodes, leaves, entries = source.native.index_wire.tree(raw, index['root'], table['definition'])
            assert len(nodes) == len(leaves) == 1
            assert entries == [key + root.to_bytes(4, 'big') for key, root, identity in pairs]
            if index['physical'] in handled:
                continue
            handled.add(index['physical'])
            previous_nodes, previous_leaves, previous_entries = source.native.index_wire.tree(
                original, old_index['root'], old['definition'])
            if entries == previous_entries:
                assert raw[index['root'] * PAGE:(index['root'] + 1) * PAGE] == original[index['root'] * PAGE:(index['root'] + 1) * PAGE]
                continue
            base = index['root'] * PAGE
            old_active = previous_nodes[index['root']]['active']
            active = nodes[index['root']]['active']
            allow(base + 2, base + 4)
            allow(base + 24, base + 26)
            allow(base + BITMAP_START, base + ENTRY_START + max(old_active, active))
            scratch = sum(raw[offset] != original[offset]
                          for offset in range(base + ENTRY_START + active,
                                              base + ENTRY_START + max(old_active, active)))
            index_frames.append(dict(table=name, physical=index['physical'], root=index['root'],
                                     entries=len(entries), history=index['history'],
                                     old_prefix=previous_nodes[index['root']]['prefix'],
                                     prefix=nodes[index['root']]['prefix'], inactive_span_differences=scratch))
    for offset, (old, new) in enumerate(zip(original, raw, strict=True)):
        assert old == new or allowed[offset], ('Protected source byte changed', offset, old, new)
    declarations = {name:source.native.declarations.declarations(raw, table) for name, table in tables.items()}
    assert declarations == before['declarations'] or all(
        {key:value for key, value in declarations[name].items() if key != 'definition_sha256'} ==
        {key:value for key, value in before['declarations'][name].items() if key != 'definition_sha256'}
        for name in declarations)
    catalog = source.catalog_long_values(raw, tables, declarations, maps, before['claims'])
    assert catalog == before['catalog_long_values']
    return dict(verified=True, deleted={name:sorted(rows) for name, rows in deleted.items() if rows},
                nulled=[list(item) for item in sorted(nulls)], null_payloads=null_payload_differences,
                physical_frames=index_frames, allocation_changes=0, history_changes=0,
                protected_original_prefix=True, unchanged_Sentinel=True,
                source_sha256=hashlib.sha256(original).hexdigest(), sha256=hashlib.sha256(raw).hexdigest())
