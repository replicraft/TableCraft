"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def render(
    original,
    entries,
    children,
    tail,
    definition,
    previous=0,
    next_=0,
    *,
    PAGE,
    common,
    put,
):
    node = children is not None
    assert entries and all(len(entry) == 9 for entry in entries)
    seed = 0 if original is None else int.from_bytes(original[24:26], 'little')
    best = common(entries)
    prefix = min(seed, best)
    full = len(entries) * (13 if node else 9)
    active = full - (len(entries) - 1) * prefix
    if active > 3616:
        prefix = best
        active = full - (len(entries) - 1) * prefix
    assert active <= 3616
    page = bytearray(PAGE) if original is None else bytearray(original)
    page[:2] = bytes([3 if node else 4, 1])
    put(page, 2, 3616 - active)
    put(page, 4, definition, 4)
    put(page, 12, previous, 4)
    put(page, 16, next_, 4)
    put(page, 20, tail, 4)
    put(page, 24, prefix)
    page[26] = 1 if node else 0
    page[27:480] = bytes(453)
    end = 0
    for position, entry in enumerate(entries):
        suffix = entry if position == 0 else entry[prefix:]
        page[480 + end:480 + end + len(suffix)] = suffix
        end += len(suffix)
        if node:
            page[480 + end:480 + end + 4] = children[position].to_bytes(4, 'big')
            end += 4
        page[27 + end // 8] |= 1 << (end % 8)
    assert end == active
    if original is None:
        assert page[480 + active:] == bytes(3616 - active)
    else:
        assert page[480 + active:] == original[480 + active:]
    return page
