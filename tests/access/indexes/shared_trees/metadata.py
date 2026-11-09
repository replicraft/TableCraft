"""Shared metadata-only tree traversal; raw decoding remains with its owner."""

def tree(pages, root):
    visited = []
    levels = {}

    def walk(number, height):
        assert 0 <= height < 16
        assert number not in visited
        visited.append(number)
        page = pages[number]
        assert page['height'] == height
        levels.setdefault(height, []).append(number)
        entries = list(map(bytes.fromhex, page['entries_hex']))
        if height == 0:
            assert page['kind'] == 4 and page['tail'] == 0
            return entries
        assert page['kind'] == 3
        result = []
        for entry in entries:
            child = walk(int.from_bytes(entry[-4:], 'big'), height - 1)
            assert child and child[-1] == entry[:-4]
            result.extend(child)
        if page['tail']:
            result.extend(walk(page['tail'], height - 1))
        return result

    rows = walk(root, pages[root]['height'])
    for level in levels.values():
        for position, number in enumerate(level):
            page = pages[number]
            assert page['previous'] == (level[position - 1] if position else 0)
            assert page['next'] == (level[position + 1] if position + 1 < len(level) else 0)
    return visited, rows
