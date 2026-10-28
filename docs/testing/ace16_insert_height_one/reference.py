"""Request-bound public fitted reference, independent of future native policy."""
from pathlib import Path
from ace16_insert_root_split import reference as source_reference

ROOT = Path(__file__).resolve().parents[3]


def before(case):
    parser = source_reference
    return parser.inspect(case['previous_selector'], ROOT / case['source']['path'],
                          ROOT / case['source_getter']['path'], ROOT / case['source_maps']['path'], True)


def word(raw, at, value, width=2):
    raw[at:at + width] = value.to_bytes(width, 'little')


def transition(case, state, image, getter, maps_path):
    parser = source_reference
    original, old, layout, old_maps, old_nodes, old_leaves = state
    raw = image.read_bytes()
    assert len(raw) == len(original), 'Fitted reference must not allocate pages'
    assert raw[:4096] == original[:4096], 'Database header changed'
    table = parser.parsed(getter.read_text())
    assert parser.schema(raw, table, 453) == layout
    assert set(table['rows']) == set(old['rows']) | {227}
    for identity, row in old['rows'].items():
        assert table['rows'][identity] == row
        assert table['raw_rows'][identity] == old['raw_rows'][identity]
    assert table['columns'] == old['columns'] and table['properties'] == old['properties']
    added = table['rows'][227]
    assert (added['page'], added['slot']) == (case['expected']['DATA_page'], case['expected']['slot'])
    binary = bytes.fromhex(case['request']['binary_hex'])
    wanted = parser.record(227, binary)
    assert len(wanted) == (13 if case['variant'] == 'fitted' else 97)
    assert added['values'] == ['Integer:227', 'Binary:' + binary.hex()]
    assert bytes.fromhex(table['raw_rows'][227]) == wanted
    begin, end = parser.CODEC.frame(raw, added['page'], added['slot'])
    assert raw[begin:end] == wanted
    assert parser.number(raw, added['page'] * 4096 + 4) == layout['definition']
    assert parser.number(raw, added['page'] * 4096 + 12, 2) == added['slot'] + 1
    assert parser.number(raw, added['page'] * 4096 + 2, 2) == begin % 4096 - (14 + 2 * (added['slot'] + 1))
    nodes, leaves, stream = parser.INDEX.tree(raw, layout['root'], layout['definition'])
    assert set(nodes) == set(old_nodes) and leaves == old_leaves
    references = {(row['page'] << 8) | row['slot']: identity
                  for identity, row in table['rows'].items()}
    assert len(references) == 453
    assert stream == sorted(parser.CODEC.int_key(identity, ref) for ref, identity in references.items())
    assert table['indexes']['PrimaryID']['order'] == sorted(table['rows'])
    assert table['indexes']['PrimaryID']['owned_pages'] == 3
    root, left, right = layout['root'], leaves[0], leaves[1]
    assert len(nodes[left]['entries']) == 226 and len(nodes[right]['entries']) == 227
    assert nodes[root]['height'] == 1 and nodes[root]['prefix'] == 0
    assert nodes[root]['children'] == [left] and nodes[root]['tail'] == right
    assert nodes[root]['entries'] == old_nodes[root]['entries']
    for page in [root, left]:
        assert raw[page * 4096:(page + 1) * 4096] == original[page * 4096:(page + 1) * 4096]
    for page in nodes:
        assert (nodes[page]['previous'], nodes[page]['next']) == (old_nodes[page]['previous'], old_nodes[page]['next'])
    assert nodes[right]['prefix'] == 0 and nodes[right]['free'] == 1573
    assert parser.number(raw, layout['history']) == parser.number(original, layout['history']) + 1 == 453
    observed_maps = parser.maps(raw, maps_path.read_text(), layout, 227, table)
    for role, old_map in old_maps.items():
        assert observed_maps[role] == old_map, ('Map framing or membership changed', role)

    expected = bytearray(original)
    data_base = added['page'] * 4096
    expected[begin:end] = wanted
    word(expected, data_base + 14 + 2 * added['slot'], begin % 4096)
    word(expected, data_base + 12, added['slot'] + 1)
    word(expected, data_base + 2, begin % 4096 - (14 + 2 * (added['slot'] + 1)))
    for field in ['table_count', 'history']:
        word(expected, layout[field], 453, 4)
    index_base = right * 4096
    active_end = 227 * 9
    new_entry = parser.CODEC.int_key(227, (added['page'] << 8) | added['slot'])
    expected[index_base + 2514:index_base + 2523] = new_entry
    word(expected, index_base + 2, 1573)
    expected[index_base + 27 + active_end // 8] |= 1 << (active_end % 8)
    # Active header/bitmap/key changes are exact. Existing free-tail scratch is
    # classified after the semantic proof rather than borrowed as engine policy.
    differences = [at for at, (a, b) in enumerate(zip(raw, expected)) if a != b]
    allowed = set(range(index_base + 2523, index_base + 4096))
    assert set(differences) <= allowed, 'Unexpected byte change outside existing right-leaf unused tail'
    return {'verified': True, 'old_ROWS_preserved': 452, 'final_rows': 453,
            'new_ROW': added['page'] << 8 | added['slot'], 'new_record_bytes': len(wanted),
            'root': root, 'root_height': 1, 'left': left, 'right': right,
            'left_entries': 226, 'right_entries': 227, 'right_prefix': 0,
            'right_active': 2043, 'right_free': 1573, 'new_pages': 0,
            'all_four_maps_unchanged': True, 'table_counter': [452, 453],
            'historical_counter': [452, 453], 'root_and_left_bytes_unchanged': True,
            'sibling_links_unchanged': True, 'source_header_unchanged': True,
            'public_unused_tail_differences': differences,
            'future_engine_policy': 'Not inferred from observed public unused scratch'}
