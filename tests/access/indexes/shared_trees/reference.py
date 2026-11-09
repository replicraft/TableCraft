"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def tree(
    raw,
    root,
    owner,
    case,
    primary=False,
    *,
    CAPACITY,
    PAGE,
    decode_entry,
    n,
):
    nodes = {}
    levels = {}
    leaves = []

    def visit(number, expected_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[:2] in (bytes([3, 1]), bytes([4, 1]))
        assert n(page, 4) == owner
        height = page[26]
        assert height < 16 and (expected_height is None or height == expected_height)
        assert (page[0] == 4) == (height == 0)
        assert not page[27] & 1 and not page[479] & 0xfe
        ends = [end for end in range(1, CAPACITY + 1)
                if page[27 + end // 8] & (1 << (end % 8))]
        active = CAPACITY - n(page, 2, 2)
        assert active == (ends[-1] if ends else 0)
        prefix = n(page, 24, 2)
        entries = []
        children = []
        start = 0
        for end in ends:
            payload = page[480 + start:480 + end]
            if height:
                assert len(payload) >= 4
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            complete = payload if not entries else entries[-1][:prefix] + payload
            decode_entry(complete, case, primary)
            assert not entries or entries[-1] < complete
            entries.append(complete)
            start = end
        common = min(map(len, entries), default=0)
        for entry in entries[1:]:
            common = next((position for position, pair in enumerate(zip(entries[0][:common], entry))
                           if pair[0] != pair[1]), common)
        assert prefix <= common
        assert active == sum(map(len, entries)) + (4 * len(entries) if height else 0) - prefix * max(0, len(entries) - 1)
        tail = n(page, 20)
        nodes[number] = {'height': height, 'prefix': prefix, 'active': active,
                         'previous': n(page, 12), 'next': n(page, 16),
                         'tail': tail, 'entries': entries}
        levels.setdefault(height, []).append(number)
        if not height:
            assert tail == 0
            leaves.append(number)
            return entries
        if tail:
            children.append(tail)
        assert children
        result = []
        for position, child in enumerate(children):
            values = visit(child, height - 1)
            assert values
            if position < len(entries):
                assert entries[position] == values[-1]
            result.extend(values)
        assert result == sorted(result) and len(result) == len(set(result))
        return result

    entries = visit(root)
    for sequence in levels.values():
        for position, number in enumerate(sequence):
            assert nodes[number]['previous'] == (sequence[position - 1] if position else 0)
            assert nodes[number]['next'] == (sequence[position + 1] if position + 1 < len(sequence) else 0)
    return nodes, leaves, entries
