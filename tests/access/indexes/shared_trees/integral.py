"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def tree(
    raw,
    root,
    owner,
    width,
    *,
    CAPACITY,
    CHILD_BYTES,
    ENTRY,
    MASK,
    MAX_DEPTH,
    PAGE,
    ROW_BYTES,
    hashlib,
    number,
):
    seen = {}
    levels = {}

    def visit(number_value, depth):
        assert depth < MAX_DEPTH and number_value not in seen
        body = raw[number_value * PAGE:(number_value + 1) * PAGE]
        assert len(body) == PAGE and body[:2] in (b'\x03\x01', b'\x04\x01')
        assert number(body, 4) == owner and body[8:12] == bytes(4)
        height = body[26]
        leaf = body[0] == 4
        assert leaf == (height == 0)
        prefix = number(body, 24, 2)
        key_row_bytes = 1 + width + ROW_BYTES
        assert prefix <= key_row_bytes + (0 if leaf else CHILD_BYTES)
        entries = []
        previous = 0
        for end in range(1, CAPACITY + 1):
            if not body[MASK + end // 8] & (1 << (end % 8)):
                continue
            full = (entries[0][:prefix] if entries else b'') + body[ENTRY + previous:ENTRY + end]
            assert len(full) == key_row_bytes + (0 if leaf else CHILD_BYTES)
            entries.append(full)
            previous = end
        assert entries and number(body, 2, 2) == CAPACITY - previous
        current = {'height': height, 'previous': number(body, 12), 'next': number(body, 16),
                   'tail': number(body, 20), 'prefix': prefix, 'free': number(body, 2, 2),
                   'entries_hex': [entry.hex() for entry in entries],
                   'sha256': hashlib.sha256(body).hexdigest()}
        seen[number_value] = current
        levels.setdefault(height, []).append(number_value)
        if leaf:
            assert current['tail'] == 0
            return entries
        result = []
        for entry in entries:
            child = int.from_bytes(entry[-CHILD_BYTES:], 'big')
            values = visit(child, depth + 1)
            assert seen[child]['height'] + 1 == height and values[-1] == entry[:-CHILD_BYTES]
            result.extend(values)
        if current['tail']:
            values = visit(current['tail'], depth + 1)
            assert seen[current['tail']]['height'] + 1 == height
            result.extend(values)
        return result

    keys = visit(root, 0)
    for pages in levels.values():
        for position, page in enumerate(pages):
            assert seen[page]['previous'] == (pages[position - 1] if position else 0)
            assert seen[page]['next'] == (pages[position + 1] if position + 1 < len(pages) else 0)
    return {'pages': seen, 'keys_hex': [entry.hex() for entry in keys]}
