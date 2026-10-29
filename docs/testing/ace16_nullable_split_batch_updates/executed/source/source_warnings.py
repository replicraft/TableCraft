"""Closed SOURCE-create warning grammar; all other stderr remains refused."""
from pathlib import Path

FORMAT = '%4$s\t%3$s\t%5$s%6$s%n'
MAX_LINE_BYTES = 2048
LOGGER = 'com.healthmarketscience.jackcess.Index'
TABLES = ('Items',) * 300 + ('UniqueItems',) * 300


def require_create(data, source_path):
    assert len(data) <= 16 * 1024 * 1024
    assert not data or data.endswith(b'\n')
    lines = data.splitlines(keepends=True)
    assert len(lines) == len(TABLES)
    for line, table in zip(lines, TABLES, strict=True):
        assert len(line) <= MAX_LINE_BYTES
        expected = ('WARNING\t' + LOGGER + '\tAdded duplicate index entry null'
                    + ' (Db=' + Path(source_path).name + ';Table=' + table
                    + ';Index=[PairOrder, PairAlias])\n').encode('utf-8')
        assert line == expected, (table, 'Unexpected preserved source warning')
    return dict(known_warning_records=len(lines), old_entry='null', tables=list(TABLES))
