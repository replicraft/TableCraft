"""Strict sparse Int32/Binary source facts; no subprocesses or candidate policy."""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_root_split import reference as base
from parent_index_split_originals import wire
from slot_preview_originals import facts
import delete_reference_maps as map_wire

PAGE_BYTES = 4096
MAX_IMAGE_BYTES = 64 * 1024 * 1024
MAX_ROWS = 4096
DELETED = 0x8000
RELOCATED = 0x4000
OFFSET_MASK = 0x1fff
MAX_SLOTS = 255


def parsed(text):
    seen = set()
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        assert tag in {'TABLE', 'COLUMN', 'PROPERTY', 'ROW', 'RAWROW', 'INDEX', 'INDEX_COLUMN', 'ENTRY'}
        if tag in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            count = 2 if tag == 'TABLE' else 3
            identity = tuple(fields[:count])
            assert identity not in seen, ('Duplicate getter fact', identity)
            seen.add(identity)
        if tag == 'COLUMN':
            assert fields[7] in ('true', 'false')
        if tag == 'INDEX':
            assert all(value in ('true', 'false') for value in fields[7:11])
        if tag == 'INDEX_COLUMN':
            assert fields[-1] in ('true', 'false')
    tables = facts.parse(text)
    assert set(tables) == {'BinaryStorage', 'Sentinel'}
    sentinel = tables['Sentinel']
    assert sentinel['count'] == 1 and set(sentinel['rows']) == {91}
    assert list(sentinel['columns']) == ['ID', 'Marker'] and not sentinel['indexes']
    assert sentinel['rows'][91]['values'] == ['Integer:91', 'Integer:12345']
    return tables


def storage(raw, text, layout):
    seen = set()
    for line in text.splitlines():
        fields = line.split('\t')
        identity = tuple(fields) if fields[0] == 'MEMBER' else tuple(fields[:2])
        assert identity not in seen, ('Duplicate map fact', identity)
        seen.add(identity)
    description = map_wire.parse(text)
    assert not description['indexes']
    assert description['page_bytes'] == PAGE_BYTES
    assert description['page_count'] == len(raw) // PAGE_BYTES
    assert set(description['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    result = {}
    for label, key in (
        ('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'),
        ('TableFreeSpace', 'hint'),
    ):
        public = map_wire.decode_map(raw, description['maps'][label], PAGE_BYTES,
                                     len(raw) // PAGE_BYTES)
        direct = base.CODEC.map_record(raw, layout[key])
        assert direct['range'][0] == public['row_start']
        assert {n for n in direct['members'] if n < len(raw) // PAGE_BYTES} == set(public['file_members'])
        result[key] = direct
    result['index_owned'] = base.CODEC.map_record(raw, layout['index_owned'])
    return result


def inspect(case, path, getter_path, maps_path):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= MAX_IMAGE_BYTES and len(raw) % PAGE_BYTES == 0
    assert raw[:4] == bytes([0, 1, 0, 0]) and raw[4:20] == b'Standard ACE DB\0'
    assert raw[20] == {'V2016': 5, 'V2019': 6}[case['profile']]
    assert raw[21:24] == bytes(3) and raw[62:66] == bytes.fromhex('fb8abc4e')
    tables = parsed(Path(getter_path).read_text())
    table = tables['BinaryStorage']
    count = table['count']
    assert 0 < count <= MAX_ROWS
    layout = base.schema(raw, table, count)
    maps = storage(raw, Path(maps_path).read_text(), layout)
    references = {}
    for identity, row in table['rows'].items():
        reference = (row['page'] << 8) | row['slot']
        assert reference not in references and 0 <= row['slot'] < MAX_SLOTS
        references[reference] = identity
        expected = base.record(identity, b'')
        assert row['values'] == ['Integer:' + str(identity), 'Binary:']
        assert bytes.fromhex(table['raw_rows'][identity]) == expected
        begin, end = base.CODEC.frame(raw, row['page'], row['slot'])
        assert raw[begin:end] == expected
    eof = len(raw) // PAGE_BYTES
    owned = {n for n in maps['owned']['members'] if n < eof}
    observed = {}
    visited_slots = 0
    deleted_slots = 0
    for number in sorted(owned):
        at = number * PAGE_BYTES
        page = raw[at:at + PAGE_BYTES]
        assert page[:2] == bytes([1, 1])
        assert base.number(page, 4) == layout['definition']
        slots = base.number(page, 12, 2)
        assert slots <= MAX_SLOTS
        directory_end = 14 + slots * 2
        last_start = PAGE_BYTES
        for slot in range(slots):
            word = base.number(page, 14 + slot * 2, 2)
            flags = word & ~OFFSET_MASK
            assert flags in (0, DELETED, DELETED | RELOCATED)
            start = word & OFFSET_MASK
            assert directory_end <= start <= last_start <= PAGE_BYTES
            visited_slots += 1
            if word & DELETED:
                deleted_slots += 1
            else:
                assert word & ~OFFSET_MASK == 0 and start < last_start
                reference = (number << 8) | slot
                assert reference in references
                identity = references[reference]
                assert page[start:last_start] == base.record(identity, b'')
                observed[reference] = identity
            last_start = start
        assert base.number(page, 2, 2) == last_start - directory_end
    assert observed == references
    nodes, leaves, entries = wire.tree(raw, layout['root'], layout['definition'])
    assert entries == sorted(base.CODEC.int_key(identity, reference)
                             for reference, identity in references.items())
    assert table['indexes']['PrimaryID']['order'] == sorted(table['rows'])
    assert len(nodes) == table['indexes']['PrimaryID']['owned_pages']
    assert maps['index_owned']['members'] == set(nodes)
    assert maps['hint']['members'] <= maps['owned']['members']
    assert not maps['global']['members'] & (owned | set(nodes))
    # One DATA plus three INDEX pages is the bounded root-growth envelope.
    # This proves existing representation, not permission to mutate a role.
    for number in range(eof, eof + 4):
        assert number in maps['global']['locations']
        assert number in maps['global']['members']
        assert number in maps['index_owned']['locations']
    layout['visited_slots'] = visited_slots
    layout['deleted_slots'] = deleted_slots
    layout['historical_before'] = base.number(raw, layout['history'])
    layout['live_before'] = count
    assert layout['historical_before'] >= count
    return raw, table, layout, maps, nodes, leaves
