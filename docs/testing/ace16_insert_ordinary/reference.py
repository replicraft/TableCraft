"""Public ordinary-row INSERT observation, independent of candidate image bytes."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_ordinary import source, requests as operands, row_codec, record_decode
from ace16_insert_candidates import model as storage_wire
from parent_index_split_originals import wire

PAGE = 4096
NEW_ID = 226


def inspect(case, raw_path, getter_path, maps_path, original):
    raw = Path(raw_path).read_bytes()
    assert 0 < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    tables = source.parsed(Path(getter_path).read_text())
    before_raw, before_table, layout, before_maps, before_nodes, before_leaves = original
    current = tables[case['table']]
    assert current['count'] == len(current['rows']) == len(current['raw_rows']) == 452
    assert set(current['rows']) == set(before_table['rows']) | {NEW_ID}
    assert current['columns'] == before_table['columns']
    assert current['properties'] == before_table['properties']
    physical = row_codec.Layout.bind(raw, current)
    for label, index in current['indexes'].items():
        old = before_table['indexes'][label]
        for key in ('physical', 'root', 'primary', 'unique', 'required', 'ignored_null', 'columns'):
            assert index[key] == old[key]
        assert index['entries'] == 452 and index['owned_pages'] == 3
        assert index['order'] == sorted(current['rows'])
    assert set(current['indexes']) == set(before_table['indexes'])
    for identity, old in before_table['rows'].items():
        assert current['rows'][identity] == old
        assert current['raw_rows'][identity] == before_table['raw_rows'][identity]
    references = {}
    for identity, row in current['rows'].items():
        reference = row['page'] << 8 | row['slot']
        assert reference not in references
        references[reference] = identity
        body = bytes.fromhex(current['raw_rows'][identity])
        begin, end = storage_wire.frame(raw, row['page'], row['slot'])
        assert raw[begin:end] == body
        assert source.number(raw, row['page'] * PAGE + 4) == layout['definition']
        decoded = record_decode.decode(physical, body)
        wanted = operands.values(case['table'] == 'MixedRows') if identity == NEW_ID else source.expected_values(identity, case['table'] == 'MixedRows')
        record_decode.same_values(decoded, wanted)
        record_decode.check_getter(physical, decoded, row['values'])
    nodes, leaves, entries = wire.tree(raw, layout['root'], layout['definition'])
    assert entries == sorted(storage_wire.int_key(identity, reference)
                             for reference, identity in references.items())
    assert len(nodes) == 3 and len(leaves) == 2
    assert nodes[layout['root']]['height'] == 1
    assert [len(nodes[number]['entries']) for number in leaves] == [226, 226]
    maps = source.storage(raw, Path(maps_path).read_text(), layout)
    assert maps['index_owned']['members'] == set(nodes)
    before_all = source.parsed((ROOT / case['before_getter']).read_text())
    for label in set(tables) - {case['table']}:
        assert tables[label] == before_all[label]
    return raw, current, layout, maps, nodes, leaves


def put(raw, at, value, width=4):
    raw[at:at + width] = value.to_bytes(width, 'little')


def transition(case, before, after):
    original, table, layout, maps, nodes, leaves = before
    current, result, _, future_maps, future_nodes, future_leaves = after
    assert len(current) >= len(original)
    expected = bytearray(original)
    assert source.number(current, layout['table_count']) == source.number(original, layout['table_count']) + 1 == 452
    assert source.number(current, layout['history']) == source.number(original, layout['history']) + 1
    put(expected, layout['table_count'], 452)
    put(expected, layout['history'], source.number(original, layout['history']) + 1)
    row = result['rows'][NEW_ID]
    number, slot = row['page'], row['slot']
    body = bytes.fromhex(result['raw_rows'][NEW_ID])
    base = number * PAGE
    page = current[base:base + PAGE]
    eof = len(original) // PAGE
    new_data = set()
    if number >= eof:
        new_data.add(number)
        framed = bytearray(PAGE)
        framed[:2] = bytes([1, 1])
        put(framed, 2, PAGE - 14 - 2 - len(body), 2)
        put(framed, 4, layout['definition'])
        put(framed, 12, 1, 2)
        put(framed, 14, PAGE - len(body), 2)
        framed[PAGE - len(body):] = body
        assert slot == 0 and page == framed
        placement = 'fresh zero-initialized DATA page'
    else:
        old = original[base:base + PAGE]
        framed = bytearray(old)
        count = source.number(old, 12, 2)
        assert slot == count and count < 255
        end = PAGE if count == 0 else source.number(old, 12 + 2 * count, 2) & 0x1fff
        start = end - len(body)
        assert start >= 14 + 2 * (count + 1)
        framed[start:end] = body
        put(framed, 2, start - 14 - 2 * (count + 1), 2)
        put(framed, 12, count + 1, 2)
        put(framed, 14 + 2 * slot, start, 2)
        assert page == framed
        expected[base:base + PAGE] = page
        placement = 'ordinary appended slot on existing DATA page'
    new_index = set(future_nodes) - set(nodes)
    assert set(nodes) <= set(future_nodes) and len(new_index) == 2
    suffix = set(range(eof, len(current) // PAGE))
    assert new_index | new_data == suffix and not new_index & new_data
    assert all(page >= eof for page in new_index)
    if case['table'] == 'MixedRows':
        assert len(new_data) == 1
    else:
        assert not new_data
    assert future_maps['global']['members'] == maps['global']['members'] - suffix
    assert future_maps['owned']['members'] == maps['owned']['members'] | new_data
    assert future_maps['index_owned']['members'] == maps['index_owned']['members'] | new_index
    assert future_maps['hint']['members'] - maps['hint']['members'] <= {number}
    for removed in maps['hint']['members'] - future_maps['hint']['members']:
        source_page = current if removed == number else original
        assert source.number(source_page, removed * PAGE + 12, 2) == 255 or source.number(source_page, removed * PAGE + 2, 2) < len(body) + 2
    map_changes = {}
    for role, old in maps.items():
        new = future_maps[role]
        assert old['kind'] == new['kind'] and old['range'] == new['range']
        assert old['locations'] == new['locations']
        edits = []
        for page in sorted(old['members'] ^ new['members']):
            at, mask = old['locations'][page]
            if page in new['members']:
                expected[at] |= mask
            else:
                expected[at] &= ~mask
            edits.append({'page': page, 'offset': at, 'mask': mask, 'set': page in new['members']})
        map_changes[role] = edits
    root = layout['root']
    expected[root * PAGE:(root + 1) * PAGE] = current[root * PAGE:(root + 1) * PAGE]
    if expected != current[:len(original)]:
        offset = next(index for index, pair in enumerate(zip(expected, current, strict=False)) if pair[0] != pair[1])
        raise AssertionError(('Unrelated prefix changed', offset, expected[offset], current[offset]))
    return {'rows_before': table['count'], 'rows_after': result['count'],
            'history_before': source.number(original, layout['history']),
            'history_after': source.number(current, layout['history']),
            'new_ROW': {'page': number, 'slot': slot, 'bytes': len(body), 'placement': placement},
            'suffix': [{'page': page, 'purpose': 'DATA' if page in new_data else 'INDEX'} for page in sorted(suffix)],
            'root': {'page': root, 'height': future_nodes[root]['height'], 'prefix': future_nodes[root]['prefix'],
                     'free': future_nodes[root]['free'], 'tail': future_nodes[root]['tail']},
            'leaf_frames': [{key: future_nodes[page][key] for key in ('number', 'prefix', 'free', 'previous', 'next', 'tail')} for page in future_leaves],
            'map_changes': map_changes, 'candidate_image_authority': False}
