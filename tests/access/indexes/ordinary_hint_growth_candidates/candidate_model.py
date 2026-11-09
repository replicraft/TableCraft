"""Enlarged-source DATA/index oracle with explicit map transitions; no children."""
import hashlib
import importlib.util
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
PAGE = 4096
CAPACITY = 3616
ROWS = 900


def load_module(name, relative):
    specification = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


WIRE = load_module('combined_independent_index_wire', 'docs/testing/encoded_index_split_originals/candidate_model.py')
RECORD = load_module('combined_independent_record_wire', 'docs/testing/variable_index_split_originals/record_model.py')
ALLOCATION = load_module('hint_transition_allocation', 'docs/testing/ordinary_hint_growth_candidates/allocation_model.py')
WEIGHTS = load_module('combined_independent_weights', 'docs/testing/variable_index_split_originals/source_weights.py')


def cell(raw, metadata, number):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    stop = metadata['table_page'] * PAGE + metadata['row_end']
    block = raw[start:stop]
    if block[0] == 0:
        relative = number - int.from_bytes(block[1:5], 'little')
        assert relative >= 0 and 5 + relative // 8 < len(block)
        offset = start + 5 + relative // 8
        bit = relative % 8
    else:
        assert block[0] == 1
        pointer, relative = divmod(number, 32736)
        assert 5 + 4 * pointer <= len(block)
        bitmap = int.from_bytes(block[1 + 4 * pointer:5 + 4 * pointer], 'little')
        assert 0 < bitmap < len(raw) // PAGE
        assert raw[bitmap * PAGE:bitmap * PAGE + 4] == b'\x05\x01\x00\x00'
        offset = bitmap * PAGE + 4 + relative // 8
        bit = relative % 8
    return offset, 1 << bit


def allocation(raw, metadata):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    stop = metadata['table_page'] * PAGE + metadata['row_end']
    block = raw[start:stop]
    members = set()
    if block[0] == 0:
        base = int.from_bytes(block[1:5], 'little')
        for index, value in enumerate(block[5:]):
            for bit in range(8):
                if value & (1 << bit):
                    members.add(base + index * 8 + bit)
    else:
        assert block[0] == 1 and (len(block) - 1) % 4 == 0
        for slot in range((len(block) - 1) // 4):
            pointer = int.from_bytes(block[1 + slot * 4:5 + slot * 4], 'little')
            if pointer == 0:
                continue
            bitmap = raw[pointer * PAGE:(pointer + 1) * PAGE]
            assert bitmap[:4] == b'\x05\x01\x00\x00'
            for index, value in enumerate(bitmap[4:]):
                for bit in range(8):
                    if value & (1 << bit):
                        members.add(slot * 32736 + index * 8 + bit)
    return members


def size(entries):
    common = WIRE.common(entries)
    total = sum(map(len, entries))
    prefix = 0 if total <= CAPACITY else common
    return total - prefix * (len(entries) - 1)


def schedule(entries, allocate):
    # Fresh RIGHT first, LEFT second. A stable allocation-order worklist visits
    # appended LEFTs in the same pass and retained RIGHTs again on later passes.
    middle = len(entries) // 2
    right = allocate()
    left = allocate()
    nodes = {right: entries[middle:], left: entries[:middle]}
    work = [right, left]
    logical = [left, right]
    changed = True
    while changed:
        changed = False
        position = 0
        while position < len(work):
            number = work[position]
            subset = nodes[number]
            if size(subset) > CAPACITY:
                assert len(subset) >= 2
                middle = len(subset) // 2
                nodes[number] = subset[middle:]
                left = allocate()
                nodes[left] = subset[:middle]
                logical.insert(logical.index(number), left)
                work.append(left)
                changed = True
            position += 1
    assert all(size(nodes[number]) <= CAPACITY for number in logical)
    return nodes, logical


def expected_image(before, facts, selected):
    assert hashlib.sha256(before).hexdigest() == facts['sha256']
    assert len(before) == facts['bytes'] and len(before) % PAGE == 0
    table = facts['tables'][selected]
    eof = len(before) // PAGE
    binary = selected == 'BinaryOne'
    source = table['rows']['1']
    root_start, root_stop = WIRE.frame(before, source['page'], source['slot'])
    original_record = before[root_start:root_stop]
    assert original_record.hex() == source['body'] and len(original_record) == 43
    count = 12 if binary else 8
    fixed = 34 if binary else 30
    shape = RECORD.Shape(count, fixed, 1, 1, 0)
    payload = bytes([255]) * 255 if binary else ('z' * 255).encode('utf-16le')
    assert source['selected']['offset'] == root_start + fixed
    assert bytes.fromhex(source['selected']['wire']) == (b'\x00' if binary else b'a\x00' * 3)
    rebuilt = RECORD.rebuild(original_record, shape, payload, True)
    assert len(rebuilt) == (297 if binary else 547)
    assert rebuilt[:fixed] == original_record[:fixed]
    assert rebuilt[fixed:fixed + len(payload)] == payload
    assert rebuilt[-(2 if binary else 1):] == original_record[-(2 if binary else 1):]

    result = bytearray(before)
    allowed = bytearray(len(before))
    allocator = ALLOCATION.Allocator(before, result, facts, cell, allocation)
    hint_role = selected + ':Hint'
    hints = facts['maps'][hint_role]['file_members']
    assert hints and hints == sorted(hints)
    highest = hints[-1]
    assert WIRE.number(before, highest * PAGE + 2, 2) == 32
    assert len(rebuilt) + 2 > 32
    allocator.remove_hint(hint_role, highest)
    data_number = allocator.data(selected)
    assert data_number == eof
    data = bytearray(PAGE)
    data[:2] = b'\x01\x01'
    data[2:4] = (4080 - len(rebuilt)).to_bytes(2, 'little')
    data[4:8] = table['definition'].to_bytes(4, 'little')
    data[12:14] = b'\x01\x00'
    data[14:16] = (0x8000 | (PAGE - len(rebuilt))).to_bytes(2, 'little')
    data[-len(rebuilt):] = rebuilt
    result[eof * PAGE:(eof + 1) * PAGE] = data
    pointer = (eof << 8).to_bytes(4, 'little')
    result[root_start:root_stop] = pointer + bytes(39)
    word_at = source['page'] * PAGE + 14 + 2 * source['slot']
    word = WIRE.number(before, word_at, 2)
    assert word & 0xe000 == 0
    result[word_at:word_at + 2] = (word | 0x4000).to_bytes(2, 'little')
    allowed[root_start:root_stop] = bytes([1]) * 43
    allowed[word_at:word_at + 2] = b'\x01\x01'

    assert table['indexes']['Ascending']['physical'] == table['indexes']['Twin']['physical'] == 1
    assert facts['maps'][selected + ':Index:Ascending'] == facts['maps'][selected + ':Index:Twin']
    trees = []
    for label in ('Ascending', 'Descending'):
        metadata = table['indexes'][label]
        root = metadata['root']
        owner = table['definition']
        old_key = bytes.fromhex(metadata['capacity']['old_key'])
        new_key = bytes.fromhex(metadata['capacity']['new_key'])
        assert (old_key, new_key) == WEIGHTS.expected_encodings(selected, label == 'Descending')
        original_entries = []
        final_entries = []
        for identity in range(1, ROWS + 1):
            row = table['rows'][str(identity)]
            reference = ((row['page'] << 8) | row['slot']).to_bytes(4, 'big')
            original_entries.append(old_key + reference)
            final_entries.append((new_key if identity == 1 else old_key) + reference)
        original_entries.sort()
        final_entries.sort()
        original_decoded, original_pages = WIRE.tree(before, root, owner)
        assert original_decoded == original_entries and original_pages == [root]
        original_prefix = WIRE.number(before, root * PAGE + 24, 2)
        common = WIRE.common(final_entries)
        retained_prefix = min(original_prefix, common)
        active = sum(map(len, final_entries)) - retained_prefix * (ROWS - 1)
        if active > CAPACITY:
            active = sum(map(len, final_entries)) - common * (ROWS - 1)
        assert active > CAPACITY
        role = selected + ':Index:' + label
        nodes, logical = schedule(final_entries, lambda: allocator.index(role, metadata['physical']))
        assert len(nodes) == 3
        bounds = []
        leaves = []
        for position, number in enumerate(logical):
            subset = nodes[number]
            leaf = WIRE.page(subset, owner, logical[position - 1] if position else 0,
                             logical[position + 1] if position + 1 < len(logical) else 0, 0)
            result[number * PAGE:(number + 1) * PAGE] = leaf
            leaves.append({'page': number, 'count': len(subset), 'prefix': WIRE.number(leaf, 24, 2),
                           'active_bytes': CAPACITY - WIRE.number(leaf, 2, 2)})
            if position + 1 < len(logical):
                bounds.append(subset[-1] + number.to_bytes(4, 'big'))
        original_root = before[root * PAGE:(root + 1) * PAGE]
        result[root * PAGE:(root + 1) * PAGE] = WIRE.page(bounds, owner, 0, 0, logical[-1], original_root)
        allowed[root * PAGE:(root + 1) * PAGE] = bytes([1]) * PAGE
        decoded, visited = WIRE.tree(result, root, owner)
        assert decoded == final_entries and visited == sorted([root] + logical)
        trees.append({'physical': metadata['physical'], 'root': root, 'leaves': leaves,
                      'bounds': [entry.hex() for entry in bounds], 'tail': logical[-1], 'entries': len(decoded)})
    allocator.finish()
    edits = allocator.edits
    roles = allocator.roles
    for offset, value in edits.items():
        result[offset] = value
        allowed[offset] = 1
    for role, metadata in facts['maps'].items():
        original_members = allocation(before, metadata)
        expected_members = set(original_members)
        change_ = roles.get(role)
        if role == selected + ':Index:Twin':
            change_ = roles.get(selected + ':Index:Ascending')
        if change_:
            expected_members.difference_update(change_['removed'])
            expected_members.update(change_['added'])
        assert allocation(result, metadata) == expected_members, role
    assert all(permitted or old == result[position] for position, (old, permitted) in enumerate(zip(before, allowed, strict=True)))
    records = 0
    physical_trees = 0
    for name, other in facts['tables'].items():
        for identity, row in other['rows'].items():
            begin, end = WIRE.frame(before, row['page'], row['slot'])
            assert before[begin:end].hex() == row['body']
            if name == selected and identity == '1':
                assert result[begin:end] == pointer + bytes(39)
                assert result[eof * PAGE + PAGE - len(rebuilt):(eof + 1) * PAGE] == rebuilt
            else:
                assert result[begin:end] == before[begin:end]
            records += 1
        seen = set()
        for index in other['indexes'].values():
            if index['physical'] in seen:
                continue
            seen.add(index['physical'])
            physical_trees += 1
            entries, pages = WIRE.tree(result, index['root'], other['definition'])
            assert len(entries) == other['count']
            if name != selected or index['physical'] == 0:
                old_entries, old_pages = WIRE.tree(before, index['root'], other['definition'])
                assert entries == old_entries and pages == old_pages
                assert all(result[page * PAGE:(page + 1) * PAGE] == before[page * PAGE:(page + 1) * PAGE] for page in pages)
    assert records == sum(table['count'] for table in facts['tables'].values())
    assert physical_trees == 10
    mixed = next(leaf for tree in trees for leaf in tree['leaves']
                 if tree['physical'] == 2 and leaf['active_bytes'] == max(item['active_bytes'] for item in tree['leaves']))
    if selected == 'TextModern':
        assert mixed['prefix'] == 2 and mixed['active_bytes'] == 3202
    return bytes(result), {
        'original_records_checked': records, 'physical_trees_checked': physical_trees,
        'logical_root': (source['page'] << 8) | source['slot'],
        'selected_original_offset': source['selected']['offset'],
        'selected_original_span': source['selected']['length'],
        'old_record_capacity': 43, 'new_record_capacity': len(rebuilt),
        'DATA_page': eof, 'DATA_hidden_slot': 0, 'DATA_free': 4080 - len(rebuilt),
        'typed_suffix_purposes': allocator.purposes,
        'bitmap_metadata_pages': [item['page'] for item in allocator.purposes if item['kind'].endswith('BITMAP')],
        'trees': trees,
        'map_bytes': [{'offset': offset, 'before': before[offset], 'after': value} for offset, value in sorted(edits.items())],
        'whole_original_prefix_checked': len(before),
        'output_bytes': len(result), 'output_sha256': hashlib.sha256(result).hexdigest(),
    }
