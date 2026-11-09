"""Pure frozen-bundle replay and model-derived fresh getter interpretation."""
from copy import deepcopy
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_ordinary import image_model, record_decode, requests, row_codec, source


PLAN = '.cache/ace16-insert-ordinary-expected-second-20261028/plan.json'
FREEZE = '.cache/ace16-insert-ordinary-expected-second-20261028/freeze.json'
PLAN_SHA256 = 'b743323b0a6bcce358919a0f7b884747ba46ee054bbb049dca45dec132d5c0b5'
FREEZE_SHA256 = '5d0cbb7364aa2e75a7c3eecf616da85a75030aa7d740e7b60d000cc5d41b35af'
SELECTORS = ['V2016-FixedRows', 'V2016-MixedRows', 'V2019-FixedRows', 'V2019-MixedRows']


def verify_bundle():
    store = ArtifactStore(ROOT)
    assert store.pin(PLAN)['sha256'] == PLAN_SHA256
    assert store.pin(FREEZE)['sha256'] == FREEZE_SHA256
    plan, frozen = store.load(PLAN), store.load(FREEZE)
    for pin in frozen['inputs'] + frozen['outputs']:
        assert store.pin(pin['path']) == pin, pin['path']
    assert [case['id'] for case in plan['cases']] == SELECTORS
    states = []
    for case in plan['cases']:
        state = source.inspect(case, store.path(case['source']),
                               store.path(case['getter']), store.path(case['maps']))
        before_tables = source.parsed(store.path(case['getter']).read_text())
        states.append((state, before_tables))
    for case, (state, _) in zip(plan['cases'], states, strict=True):
        image, details = image_model.derive(state, [requests.values(case['table'] == 'MixedRows')],
                                            state[2]['id'])
        assert image == store.path(case['expected']['path']).read_bytes()
        assert details == case['summary'] == store.load(case['details']['path'])
        assert store.path(case['requests']['path']).read_bytes() == requests.tsv(case['table'] == 'MixedRows')
    return plan, states


def verify_fresh(image, case, state, before_tables, getter_text, maps_text):
    """Caller compares entire actual candidate to the expected image first."""
    tree = image_model.tree
    _, original_table, layout, _, _, _ = state
    tables = source.parsed(getter_text)
    assert set(tables) == set(before_tables)
    selected = case['table']
    for name, original in before_tables.items():
        if name != selected:
            assert tables[name] == original, ('Unselected table', name)
    table = tables[selected]
    for name in ('definition', 'columns_count', 'max_columns', 'columns', 'properties'):
        assert table[name] == original_table[name], ('Selected metadata', name)
    details = case['summary']
    assert table['count'] == details['final_rows'] == 452
    assert set(table['rows']) == set(original_table['rows']) | {226}
    assert set(table['raw_rows']) == set(table['rows'])
    for identity, original in original_table['rows'].items():
        assert table['rows'][identity] == original
        assert table['raw_rows'][identity] == original_table['raw_rows'][identity]
    row = table['rows'][226]
    reference = details['new_ROWS'][0]
    assert (row['page'], row['slot']) == (reference >> 8, reference & 255)
    begin, end = tree.WIRE.frame(image, row['page'], row['slot'])
    body = image[begin:end]
    assert table['raw_rows'][226] == body.hex()
    physical = row_codec.Layout.bind(image, table)
    decoded = record_decode.decode(physical, body)
    record_decode.same_values(decoded, requests.values(selected == 'MixedRows'))
    record_decode.check_getter(physical, decoded, row['values'])
    top, nodes, levels = tree.decode_tree(image, layout['root'], layout['definition'])
    assert top.height == details['root_height_after'] == 1
    expected_indexes = deepcopy(original_table['indexes'])
    for index in expected_indexes.values():
        index['entries'] += 1
        index['owned_pages'] = len(nodes)
        index['order'] = sorted(table['rows'])
    assert table['indexes'] == expected_indexes
    wires = [entry for page in levels[0] for entry in nodes[page].entries]
    assert wires == sorted(tree.key(identity, item['page'] << 8 | item['slot'])
                           for identity, item in table['rows'].items())
    maps = source.storage(image, maps_text, layout)
    assert maps['index_owned']['members'] == set(nodes)
    assert maps['hint']['members'] <= maps['owned']['members']
    assert not maps['global']['members'] & (maps['owned']['members'] | set(nodes))
    assert tree.number(image, layout['table_count']) == 452
    assert tree.number(image, layout['history']) == details['history_after'] == 452
    return {'verified': True, 'typed_rows': sum(item['count'] for item in tables.values()),
            'selected_rows': table['count'], 'selected_primary_key_ROW_pairs': len(wires),
            'old_selected_ROWS_preserved': len(original_table['rows']),
            'new_ROWS': [reference], 'table_inventory': sorted(tables),
            'public_AFTER_stream_or_ROW_authority': False}
