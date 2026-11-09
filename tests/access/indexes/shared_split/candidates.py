"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def decode_tree(
    raw,
    root,
    owner,
    width,
    unsigned_byte,
    descending,
    *,
    BITMAP,
    CAPACITY,
    CHILD_BYTES,
    ENTRY,
    FREE,
    HEIGHT,
    NEXT,
    OWNER,
    PAGE,
    PREFIX,
    PREVIOUS,
    ROW_BYTES,
    TAIL,
    decode_key,
    number,
):
    pages = {}
    leaves = []

    def visit(number_value, depth):
        assert depth < 16 and number_value not in pages
        page = raw[number_value * PAGE:(number_value + 1) * PAGE]
        assert len(page) == PAGE and page[1] == 1
        assert number(page, OWNER) == owner
        leaf = page[0] == 4
        assert page[0] in (3, 4) and leaf == (page[HEIGHT] == 0)
        prefix = number(page, PREFIX, 2)
        assert prefix <= 1 + width + ROW_BYTES
        entries = []
        used = 0
        for end in range(1, CAPACITY + 1):
            if not page[BITMAP + end // 8] & (1 << (end % 8)):
                continue
            payload = page[ENTRY + used:ENTRY + end]
            full = payload if not entries else entries[0][:prefix] + payload
            assert len(full) == 1 + width + ROW_BYTES + (0 if leaf else CHILD_BYTES)
            entries.append(full)
            used = end
        assert entries and number(page, FREE, 2) == CAPACITY - used
        pages[number_value] = {
            'height': page[HEIGHT], 'previous': number(page, PREVIOUS),
            'next': number(page, NEXT), 'tail': number(page, TAIL),
            'entries': entries,
        }
        if leaf:
            assert number(page, TAIL) == 0
            leaves.append(number_value)
            return entries
        keys = []
        for bound in entries:
            child = int.from_bytes(bound[-CHILD_BYTES:], 'big')
            nested = visit(child, depth + 1)
            assert nested[-1] == bound[:-CHILD_BYTES]
            assert pages[child]['height'] + 1 == page[HEIGHT]
            keys.extend(nested)
        if number(page, TAIL):
            child = number(page, TAIL)
            nested = visit(child, depth + 1)
            assert pages[child]['height'] + 1 == page[HEIGHT]
            keys.extend(nested)
        return keys

    entries = visit(root, 0)
    assert entries == sorted(entries)
    for position, page in enumerate(leaves):
        assert pages[page]['previous'] == (leaves[position - 1] if position else 0)
        assert pages[page]['next'] == (leaves[position + 1] if position + 1 < len(leaves) else 0)
    decoded = [decode_key(entry, width, unsigned_byte, descending) for entry in entries]
    return pages, leaves, decoded, entries
