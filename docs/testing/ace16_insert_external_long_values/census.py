"""Raw catalog roles and all live long-value references, including Legacy storage."""
PAGE = 4096


def prove(raw, tables, declarations, maps, claims, selected, wire):
    protected = {}
    owned_claims = {}
    live_claims = {}
    long_tables = {}

    def protect(page, role):
        assert 0 <= page < len(raw) // PAGE
        protected.setdefault(page, set()).add(role)

    for role, claim in claims.items():
        protect(claim['container'], role + ':Container')
        for page in claim['bitmaps']:
            protect(page, role + ':Bitmap')
    for name, table in tables.items():
        declaration = declarations[name]
        owner = 'Table:' + str(table['definition'])
        for page in declaration['pages']:
            protect(page, owner + ':Definition')
        for role, description in maps.items():
            if role == owner + ':Owned' or role.startswith(owner + ':IndexData:'):
                for page in description['file_members']:
                    protect(page, role)
        for pair in declaration['pairs']:
            role = owner + ':Column:' + str(pair['column']) + ':Owned'
            if pair['owned']:
                assert role in maps
                for page in maps[role]['file_members']:
                    owned_claims.setdefault(page, set()).add((table['definition'], pair['column']))
        columns = []
        for block in declaration['columns']:
            if block['native'] in (11, 12):
                header = bytes.fromhex(block['raw_hex'])
                columns.append({'number': block['number'], 'native_type': block['native'],
                                'variable': int.from_bytes(header[7:9], 'little')})
        if not columns:
            continue
        count = 0
        traced = 0
        for page in maps[owner + ':Owned']['file_members']:
            base = page * PAGE
            body = raw[base:base + PAGE]
            assert body[:2] == b'\x01\x01'
            assert int.from_bytes(body[4:8], 'little') == table['definition']
            slots = int.from_bytes(body[12:14], 'little')
            assert slots <= 255
            for slot in range(slots):
                word = int.from_bytes(body[14 + slot * 2:16 + slot * 2], 'little')
                assert word & 0x2000 == 0
                if word & 0x8000:
                    continue
                frame, chain = wire.final_frame(raw, {'page': page, 'slot': slot})
                for position, part in enumerate(chain):
                    at = part['page'] * PAGE
                    assert raw[at:at + 2] == b'\x01\x01'
                    assert int.from_bytes(raw[at + 4:at + 8], 'little') == table['definition']
                    flags = part['word'] & 0xe000
                    if position + 1 < len(chain):
                        assert flags == 0x4000
                    else:
                        assert flags in ((0, 0x8000) if position else (0,))
                record = raw[frame['start']:frame['end']]
                row_columns = int.from_bytes(record[:2], 'little')
                mask = len(record) - (row_columns + 7) // 8
                for column in columns:
                    number = column['number']
                    if number >= row_columns or not record[mask + number // 8] & (1 << (number % 8)):
                        continue
                    value = wire.long_value(raw, record, column)
                    for chunk in value['chunks']:
                        live_claims.setdefault(chunk['page'], set()).add((table['definition'], number))
                        traced += 1
                count += 1
        assert count == table['count'], (name, count, table['count'])
        long_tables[name] = {'live_parent_rows': count, 'live_chunk_references': traced,
                             'declared_map_columns': len(declaration['pairs'])}

    selected_definition = tables['Items']['definition']
    for name, scope in selected.items():
        column = 2 if name == 'Blob' else 3
        identity = (selected_definition, column)
        for page in scope['pages']:
            assert page not in protected, (name, page, protected.get(page))
            assert owned_claims[page] == {identity}
            assert live_claims.get(page, set()) <= {identity}
        role_prefix = 'Table:' + str(selected_definition) + ':Column:' + str(column)
        for suffix in (':Owned', ':Hint'):
            role = role_prefix + suffix
            selected_claim = claims[role]
            for other, claim in claims.items():
                if other != role:
                    assert (selected_claim['row_start'], selected_claim['row_end']) != (
                        claim['row_start'], claim['row_end']), (role, other)
    return {
        'protected_pages': {str(page): sorted(roles) for page, roles in protected.items()},
        'long_tables': long_tables,
        'owned_claims': {str(page): sorted(roles) for page, roles in owned_claims.items()},
        'live_claims': {str(page): sorted(roles) for page, roles in live_claims.items()},
    }
