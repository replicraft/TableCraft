"""Independent source-to-whole-image oracle; no processes or production codecs."""
from pathlib import Path

PAGE = 4096
ENTRY = 480
CAPACITY = PAGE - ENTRY
MASK_START = 27
ROWS = 900


def number(raw, offset, width=4, order='little'):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, order)


def common(entries, node=False):
    assert entries
    omitted = 4 if node else 0
    length = min(len(entry) - omitted for entry in entries)
    for entry in entries[1:]:
        length = next((position for position in range(length)
                       if entries[0][position] != entry[position]), length)
    return length


def framing(entries, seed, node=False):
    best = common(entries, node)
    prefix = min(seed, best)
    total = sum(map(len, entries))
    if total - prefix * (len(entries) - 1) > CAPACITY:
        prefix = best
    length = total - prefix * (len(entries) - 1)
    assert length <= CAPACITY
    return prefix, length


def decode(raw, page):
    block = raw[page * PAGE:(page + 1) * PAGE]
    assert len(block) == PAGE and block[:2] in (b'\x03\x01', b'\x04\x01')
    prefix = number(block, 24, 2)
    previous = 0
    entries = []
    for end in range(1, CAPACITY + 1):
        if not block[MASK_START + end // 8] & (1 << (end % 8)):
            continue
        wire = (entries[0][:prefix] if entries else b'') + block[ENTRY + previous:ENTRY + end]
        assert prefix <= len(entries[0]) if entries else True
        assert len(wire) >= (8 if block[0] == 3 else 4)
        entries.append(wire)
        previous = end
    assert previous + number(block, 2, 2) == CAPACITY
    return entries


def page(entries, owner, previous, next_page, tail, original=None):
    node = tail != 0
    result = bytearray(PAGE) if original is None else bytearray(original)
    assert len(result) == PAGE
    result[:2] = bytes((3 if node else 4, 1))
    result[4:8] = owner.to_bytes(4, 'little')
    result[12:16] = previous.to_bytes(4, 'little')
    result[16:20] = next_page.to_bytes(4, 'little')
    result[20:24] = tail.to_bytes(4, 'little')
    result[26] = int(node)
    result[MASK_START:ENTRY] = bytes(ENTRY - MASK_START)
    prefix, length = framing(entries, 0, node)
    result[24:26] = prefix.to_bytes(2, 'little')
    result[2:4] = (CAPACITY - length).to_bytes(2, 'little')
    end = 0
    for ordinal, wire in enumerate(entries):
        suffix = wire[0 if ordinal == 0 else prefix:]
        result[ENTRY + end:ENTRY + end + len(suffix)] = suffix
        end += len(suffix)
        result[MASK_START + end // 8] |= 1 << (end % 8)
    assert end == length
    assert decode(result, 0) == entries
    if original is None:
        assert not any(result[ENTRY + length:])
    else:
        assert result[ENTRY + length:] == original[ENTRY + length:]
    return result


def cell(raw, metadata, page_number):
    start = metadata['table_page'] * PAGE + metadata['row_start']
    end = metadata['table_page'] * PAGE + metadata['row_end']
    assert end - start == 69 and raw[start] == 0
    base = number(raw, start + 1)
    relative = page_number - base
    assert 0 <= relative < (end - start - 5) * 8
    return start + 5 + relative // 8, 1 << (relative % 8)


def frame(raw, page_number, slot):
    base = page_number * PAGE
    assert raw[base:base + 2] == b'\x01\x01'
    count = number(raw, base + 12, 2)
    assert 0 <= slot < count <= 255
    word = number(raw, base + 14 + slot * 2, 2)
    assert word & 0xe000 == 0
    start = base + word
    end = base + (number(raw, base + 12 + slot * 2, 2) if slot else PAGE)
    assert base + 14 + count * 2 <= start < end <= base + PAGE
    return start, end


def selected_span(raw, table):
    assert list(table['rows']) == [str(identity) for identity in range(1, ROWS + 1)]
    rows = table['rows']
    physical = sorted((row['page'], row['slot'], int(identity)) for identity, row in rows.items())
    assert [identity for _, _, identity in physical] == list(range(1, ROWS + 1))
    first = rows['1']
    start, end = frame(raw, first['page'], first['slot'])
    body = raw[start:end]
    assert body.hex() == first['body']
    assert number(body, 0, 2) == 3 and body[-1] & 7 == 7
    assert table['columns']['KeyValue']['variable'] == 0
    assert (table['columns']['KeyValue']['flags'], table['columns']['KeyValue']['extended']) == (2, 0)
    assert number(body, len(body) - 3, 2) == 1
    begin = number(body, len(body) - 5, 2)
    stop = number(body, len(body) - 7, 2)
    assert begin == 10 and 10 <= begin <= stop <= len(body) - 7
    assert first['fixed']['ID'] == {'begin': 2, 'end': 6, 'value': 1}
    assert first['fixed']['Marker'] == {'begin': 6, 'end': 10, 'value': 17}
    assert body[2:6] == (1).to_bytes(4, 'little')
    assert body[6:10] == (17).to_bytes(4, 'little')
    assert first['selected'] == {'offset': start + begin, 'length': stop - begin,
                                 'wire': body[begin:stop].hex()}
    return start + begin, stop - begin


def tree(raw, root, owner):
    visited = set()
    levels = {}

    def walk(number_, height):
        assert number_ not in visited and 0 <= height < 16
        visited.add(number_)
        block = raw[number_ * PAGE:(number_ + 1) * PAGE]
        assert number(block, 4) == owner and block[26] == height
        levels.setdefault(height, []).append(number_)
        entries = decode(raw, number_)
        if height == 0:
            assert block[0] == 4 and number(block, 20) == 0
            return entries
        assert block[0] == 3
        result = []
        for bound in entries:
            child = walk(int.from_bytes(bound[-4:], 'big'), height - 1)
            assert child and bound[:-4] == child[-1]
            result.extend(child)
        tail = number(block, 20)
        if tail:
            result.extend(walk(tail, height - 1))
        return result

    entries = walk(root, raw[root * PAGE + 26])
    for pages in levels.values():
        for position, number_ in enumerate(pages):
            assert number(raw, number_ * PAGE + 12) == (pages[position - 1] if position else 0)
            assert number(raw, number_ * PAGE + 16) == (pages[position + 1] if position + 1 < len(pages) else 0)
    return entries, sorted(visited)


def expected_image(before, facts, selected):
    assert len(before) == facts['bytes'] and len(before) % PAGE == 0
    table = facts['tables'][selected]
    assert table['count'] == ROWS
    offset, width = selected_span(before, table)
    binary = selected == 'BinaryOne'
    original_payload = b'\x00' if binary else 'aaa'.encode('utf-16le')
    replacement = b'\xff' if binary else 'zzz'.encode('utf-16le')
    assert width == len(original_payload) == len(replacement)
    assert before[offset:offset + width] == original_payload
    result = bytearray(before)
    result.extend(bytes(6 * PAGE))
    result[offset:offset + width] = replacement
    edits = {}
    spans = [(offset, offset + width)]
    expected_trees = []
    eof = len(before) // PAGE
    assert table['indexes']['Ascending']['physical'] == table['indexes']['Twin']['physical'] == 1
    assert table['indexes']['Descending']['physical'] == 2
    assert facts['maps'][f'{selected}:Index:Ascending'] == facts['maps'][f'{selected}:Index:Twin']
    for physical, label in enumerate(('Ascending', 'Descending')):
        metadata = table['indexes'][label]
        assert metadata['physical'] == physical + 1 and metadata['height'] == 0
        root = metadata['root']
        owner = table['definition']
        old = bytes.fromhex(metadata['capacity']['old_key'])
        new = bytes.fromhex(metadata['capacity']['new_key'])
        original_entries = []
        final_entries = []
        for identity in range(1, ROWS + 1):
            row = table['rows'][str(identity)]
            reference = ((row['page'] << 8) | row['slot']).to_bytes(4, 'big')
            original_entries.append(old + reference)
            final_entries.append((new if identity == 1 else old) + reference)
        original_entries.sort(key=lambda entry: (entry[:-4], entry[-4:]))
        final_entries.sort(key=lambda entry: (entry[:-4], entry[-4:]))
        decoded, original_pages = tree(before, root, owner)
        assert decoded == original_entries and original_pages == [root]
        assert root == table['indexes']['Twin']['root'] if physical == 0 else True
        full_bytes = sum(map(len, final_entries)) - common(final_entries) * (ROWS - 1)
        assert full_bytes > CAPACITY
        base = eof + physical * 3
        descending = label == 'Descending'
        pages = [base + 2, base + 1, base] if descending else [base + 1, base + 2, base]
        counts = [225, 225, 450] if descending else [450, 225, 225]
        position = 0
        bounds = []
        leaves = []
        for ordinal, (number_, count) in enumerate(zip(pages, counts, strict=True)):
            subset = final_entries[position:position + count]
            leaf = page(subset, owner, pages[ordinal - 1] if ordinal else 0,
                        pages[ordinal + 1] if ordinal < 2 else 0, 0)
            result[number_ * PAGE:(number_ + 1) * PAGE] = leaf
            leaves.append({'page': number_, 'count': count, 'prefix': number(leaf, 24, 2),
                           'active_bytes': CAPACITY - number(leaf, 2, 2)})
            if ordinal < 2:
                bounds.append(subset[-1] + number_.to_bytes(4, 'big'))
            position += count
        original_root = before[root * PAGE:(root + 1) * PAGE]
        result[root * PAGE:(root + 1) * PAGE] = page(bounds, owner, 0, 0, base, original_root)
        spans.append((root * PAGE, (root + 1) * PAGE))
        for number_ in range(base, base + 3):
            for role, allocated in ((f'{selected}:Index:{label}', False), ('GlobalFree', True)):
                cell_offset, mask = cell(before, facts['maps'][role], number_)
                if cell_offset not in edits:
                    edits[cell_offset] = before[cell_offset]
                assert bool(before[cell_offset] & mask) == allocated
                edits[cell_offset] = (edits[cell_offset] & ~mask) if allocated else (edits[cell_offset] | mask)
        decoded, pages_ = tree(result, root, owner)
        assert decoded == final_entries and pages_ == sorted([root] + pages)
        expected_trees.append({'physical': metadata['physical'], 'root': root,
                               'leaves': leaves, 'bounds': [bound.hex() for bound in bounds],
                               'tail': base, 'count': len(decoded)})
    for offset_, value in edits.items():
        result[offset_] = value
        spans.append((offset_, offset_ + 1))
    allowed = bytearray(len(before))
    for begin, end in spans:
        allowed[begin:end] = bytes([1]) * (end - begin)
    assert all(allowed[index] or value == result[index] for index, value in enumerate(before))
    records = 0
    for name, metadata in facts['tables'].items():
        for identity, row in metadata['rows'].items():
            begin, end = frame(before, row['page'], row['slot'])
            assert before[begin:end].hex() == row['body']
            expected = bytearray(before[begin:end])
            if name == selected and identity == '1':
                expected[offset - begin:offset - begin + width] = replacement
            assert result[begin:end] == expected
            records += 1
        checked = set()
        for index in metadata['indexes'].values():
            if index['physical'] in checked:
                continue
            checked.add(index['physical'])
            after_entries, pages_ = tree(result, index['root'], metadata['definition'])
            assert len(after_entries) == metadata['count']
            if name != selected or index['physical'] == 0:
                before_entries, before_pages = tree(before, index['root'], metadata['definition'])
                assert after_entries == before_entries and pages_ == before_pages
                assert all(result[p * PAGE:(p + 1) * PAGE] == before[p * PAGE:(p + 1) * PAGE] for p in pages_)
    assert records == 2701
    return bytes(result), {'slot_offset': offset, 'slot_width': width,
                           'original_records': records, 'physical_trees': expected_trees,
                           'map_edits': [{'offset': offset_, 'before': before[offset_], 'after': value}
                                         for offset_, value in sorted(edits.items())],
                           'allowed_original_spans': spans}
