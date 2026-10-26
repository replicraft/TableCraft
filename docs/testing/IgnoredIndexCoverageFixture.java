import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;
import java.util.Base64;
import java.util.HexFormat;

/** Original public-writer/getter corpus; physical metadata getters are identified explicitly. */
public final class IgnoredIndexCoverageFixture {
    private enum Kind {
        BOOL(DataType.BOOLEAN), BYTE(DataType.BYTE), SHORT(DataType.INT),
        INT32(DataType.LONG), INT64(DataType.BIG_INT), CURRENCY(DataType.MONEY),
        SINGLE(DataType.FLOAT), DOUBLE(DataType.DOUBLE), DATE(DataType.SHORT_DATE_TIME),
        GUID(DataType.GUID), DECIMAL(DataType.NUMERIC),
        TEXT0(DataType.TEXT), TEXT1(DataType.TEXT), MEMO0(DataType.MEMO), MEMO1(DataType.MEMO),
        BINARY(DataType.BINARY);

        final DataType type;

        Kind(DataType type) {
            this.type = type;
        }

        boolean text() {
            return type == DataType.TEXT || type == DataType.MEMO;
        }

        ColumnBuilder column(String name) {
            ColumnBuilder column = new ColumnBuilder(name, type);
            if (type == DataType.TEXT) {
                column.setLengthInUnits(255);
            } else if (type == DataType.BINARY) {
                column.setLength(255);
            } else if (type == DataType.NUMERIC) {
                column.setPrecision(28).setScale(4);
            }
            if (text()) {
                column.setTextSortOrder(this == TEXT0 || this == MEMO0
                    ? ColumnImpl.GENERAL_LEGACY_SORT_ORDER : ColumnImpl.GENERAL_SORT_ORDER);
            }
            return column;
        }
    }

    private static final int[] SINGLE_BITS = {
        0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000, 0x80800000,
        0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000,
        0x7fc00001, 0xffc00001, 0x7fc00002, 0xffc00002,
    };
    private static final long[] DOUBLE_BITS = {
        0L, Long.MIN_VALUE, 1L, 0x8000000000000001L,
        0x000fffffffffffffL, 0x800fffffffffffffL, 0x0010000000000000L, 0x8010000000000000L,
        0x3ff0000000000000L, 0xbff0000000000000L, 0x7fefffffffffffffL, 0xffefffffffffffffL,
        0x7ff0000000000000L, 0xfff0000000000000L, 0x7ff8000000000001L, 0xfff8000000000001L,
        0x7ff8000000000002L, 0xfff8000000000002L,
    };
    private static final long[] WIDE = {
        Long.MIN_VALUE, Long.MIN_VALUE + 1, -9007199254740993L, -1L, 0L, 1L,
        9007199254740992L, 9007199254740993L, Long.MAX_VALUE - 1, Long.MAX_VALUE,
    };
    private static final String[] COEFFICIENTS = {
        "0", "1", "-1", "10001", "-10001", "9007199254740993", "-9007199254740993",
        "9999999999999999999999999999", "-9999999999999999999999999999",
    };
    private static final String[] TEXT = {
        "", "a", "A", "a  ", "é", "e\u0301", "漢字", "😀", "a-b", "\u0000",
        "a".repeat(255), "\u06d6".repeat(255), "😀".repeat(127) + "x",
    };

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets authorized");
        });
    }

    private static Object value(Kind kind, int index, boolean unique) {
        int selected = unique ? index : index % 13;
        switch (kind) {
            case BOOL:
                return (selected & 1) != 0;
            case BYTE:
                return unique ? selected : new int[] {0, 1, 127, 128, 254, 255}[selected % 6];
            case SHORT:
                return (short) (unique ? selected + 100 : new int[] {-32768, -1, 0, 1, 32767}[selected % 5]);
            case INT32:
                return unique ? selected + 100 : new int[] {Integer.MIN_VALUE, -1, 0, 1, Integer.MAX_VALUE}[selected % 5];
            case INT64:
                return unique ? selected + 100L : WIDE[selected % WIDE.length];
            case CURRENCY:
                return BigDecimal.valueOf(unique ? selected + 100L : WIDE[selected % WIDE.length], 4);
            case SINGLE:
                return Float.intBitsToFloat(SINGLE_BITS[selected % SINGLE_BITS.length]);
            case DOUBLE:
                return Double.longBitsToDouble(DOUBLE_BITS[selected % DOUBLE_BITS.length]);
            case DATE:
                return LocalDateTime.of(1899, 12, 20, 0, 0).plusHours(selected);
            case GUID:
                return String.format("{%08x-1234-5678-abcd-%012x}", selected, selected * 17L);
            case DECIMAL:
                return unique ? BigDecimal.valueOf(selected + 100L, 4)
                    : new BigDecimal(new BigInteger(COEFFICIENTS[selected % COEFFICIENTS.length]), 4);
            case TEXT0:
            case TEXT1:
                return unique ? "unique" + selected : TEXT[selected % TEXT.length];
            case MEMO0:
            case MEMO1:
                return unique ? "unique" + selected : "memo" + selected + ":" + "m".repeat(800);
            case BINARY:
                if (!unique && selected == 0) {
                    return new byte[0];
                }
                byte[] bytes = new byte[unique ? 8 : 128];
                for (int offset = 0; offset < bytes.length; offset++) {
                    bytes[offset] = (byte) (selected * 17 + offset * 31);
                }
                return bytes;
            default:
                throw new AssertionError(kind);
        }
    }

    private static Table table(Database database, String name, Kind[] kinds, boolean unique) throws Exception {
        TableBuilder builder = new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG));
        String[] names = new String[kinds.length];
        for (int index = 0; index < kinds.length; index++) {
            names[index] = "K" + index;
            builder.addColumn(kinds[index].column(names[index]));
        }
        IndexBuilder ordinary = new IndexBuilder("Ordinary").addColumns(names);
        IndexBuilder ignored = new IndexBuilder("Ignored").addColumns(names).setIgnoreNulls();
        IndexBuilder alias = new IndexBuilder("IgnoredAlias").addColumns(names).setIgnoreNulls();
        IndexBuilder mixed = new IndexBuilder("IgnoredMixed").setIgnoreNulls();
        for (int index = names.length - 1; index >= 0; index--) {
            mixed.addColumns(index % 2 == 0, names[index]);
        }
        if (unique) {
            ordinary.setUnique();
            ignored.setUnique();
            alias.setUnique();
            mixed.setUnique();
        }
        return builder.addIndex(ordinary).addIndex(ignored).addIndex(alias).addIndex(mixed).toTable(database);
    }

    private static void scalar(Database database, Kind kind) throws Exception {
        Table ordinary = table(database, "Scalar" + kind, new Kind[] {kind}, false);
        int count = kind == Kind.BOOL ? 801 : kind.text() || kind == Kind.BINARY ? 201 : 701;
        for (int id = 0; id < count; id++) {
            ordinary.addRow(id, id % 11 == 0 ? null : value(kind, id * 7, false));
        }
        Table unique = table(database, "Unique" + kind, new Kind[] {kind}, true);
        int keys = kind == Kind.BOOL ? 2 : kind == Kind.SINGLE || kind == Kind.DOUBLE ? 18 : 32;
        for (int id = 0; id < keys; id++) {
            unique.addRow(id, value(kind, id, true));
        }
        if (kind != Kind.BOOL) {
            unique.addRow(keys, null);
            unique.addRow(keys + 1, null);
        }
    }

    private static void tuple(Database database, String name, Kind[] kinds, int rows) throws Exception {
        Table table = table(database, name, kinds, false);
        for (int id = 0; id < rows; id++) {
            Object[] values = new Object[kinds.length + 1];
            values[0] = id;
            for (int column = 0; column < kinds.length; column++) {
                values[column + 1] = id % 13 == 0 || (id + column) % 7 == 0
                    ? null : value(kinds[column], id * 7 + column, false);
            }
            table.addRow(values);
        }
    }

    private static void special(Database database) throws Exception {
        Table unique = table(database, "UniquePartial", new Kind[] {Kind.INT32, Kind.INT32}, true);
        Object[][] pairs = {
            {0, null, null}, {1, null, null}, {2, null, 0}, {3, 0, null}, {4, 0, 0}, {5, 1, 1},
        };
        for (Object[] pair : pairs) {
            unique.addRow(pair);
        }
        Table encoded = table(database, "UniqueEncoded", new Kind[] {Kind.TEXT1, Kind.INT32}, true);
        Object[][] encodedRows = {
            {0, null, null}, {1, null, null}, {2, null, 0}, {3, "a", null}, {4, "b", 0},
        };
        for (Object[] row : encodedRows) {
            encoded.addRow(row);
        }
        Table digest = table(database, "UniqueDigest", new Kind[] {Kind.TEXT1, Kind.BINARY}, true);
        digest.addRow(0, "a".repeat(255), new byte[255]);

        Table evolved = new TableBuilder("EvolvedPair")
            .addColumn(new ColumnBuilder("ID", DataType.LONG)).toTable(database);
        for (int id = 0; id < 17; id++) {
            evolved.addRow(id);
        }
        new ColumnBuilder("A", DataType.LONG).addToTable(evolved);
        new ColumnBuilder("B", DataType.LONG).addToTable(evolved);
        evolved.addRow(17, null, 0);
        evolved.addRow(18, 0, null);
        evolved.addRow(19, 1, 1);
        evolved.addRow(20, null, null);
        new IndexBuilder("Ordinary").addColumns("A", "B").setUnique().addToTable(evolved);
        new IndexBuilder("Ignored").addColumns("A", "B").setUnique().setIgnoreNulls().addToTable(evolved);
        new IndexBuilder("IgnoredMixed").addColumns(false, "B").addColumns("A")
            .setUnique().setIgnoreNulls().addToTable(evolved);
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        if (!profile.equals("V2016") && !profile.equals("V2019")) {
            throw new IllegalArgumentException("ACE16 priority profiles required");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            for (Kind kind : Kind.values()) {
                scalar(database, kind);
            }
            tuple(database, "TupleCompact", new Kind[] {
                Kind.BYTE, Kind.SHORT, Kind.INT32, Kind.INT64, Kind.CURRENCY,
                Kind.SINGLE, Kind.DOUBLE, Kind.DATE,
            }, 201);
            tuple(database, "TupleGuid", new Kind[] {Kind.GUID, Kind.INT32}, 201);
            tuple(database, "TupleDecimal", new Kind[] {Kind.DECIMAL, Kind.GUID, Kind.CURRENCY}, 201);
            tuple(database, "TupleTen", new Kind[] {
                Kind.BYTE, Kind.SHORT, Kind.INT32, Kind.INT64, Kind.CURRENCY,
                Kind.SINGLE, Kind.DOUBLE, Kind.DATE, Kind.GUID, Kind.DECIMAL,
            }, 201);
            tuple(database, "TupleEncoded", new Kind[] {Kind.TEXT1, Kind.BINARY, Kind.INT32}, 128);
            tuple(database, "TupleEncodedTen", new Kind[] {
                Kind.TEXT0, Kind.TEXT1, Kind.BYTE, Kind.SHORT, Kind.GUID,
                Kind.DECIMAL, Kind.BINARY, Kind.BOOL, Kind.DOUBLE, Kind.DATE,
            }, 64);
            tuple(database, "TupleMemo", new Kind[] {Kind.MEMO1, Kind.GUID, Kind.DECIMAL}, 64);
            special(database);
        }
    }

    private static String encoded(Object value, DataType type) {
        if (value == null) {
            return "NULL";
        }
        if (type == DataType.FLOAT) {
            return "F32:" + String.format("%08x", Float.floatToRawIntBits((Float) value));
        }
        if (type == DataType.DOUBLE) {
            return "F64:" + String.format("%016x", Double.doubleToRawLongBits((Double) value));
        }
        if (value instanceof BigDecimal decimal) {
            return "DEC:" + decimal.unscaledValue() + ":" + decimal.scale();
        }
        if (value instanceof byte[] bytes) {
            return "BIN:" + HexFormat.of().formatHex(bytes);
        }
        if (value instanceof String text) {
            return "TXT:" + Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
        }
        if (type == DataType.BYTE) {
            return "U8:" + (((Number) value).intValue() & 255);
        }
        return type + ":" + value;
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : database.getTableNames()) {
                Table table = database.getTable(name);
                TableImpl nativeTable = (TableImpl) table;
                System.out.printf("TABLE\t%s\t%d\t%d\t%d%n", name, table.getRowCount(),
                    nativeTable.getTableDefPageNumber(), nativeTable.getFormat().PAGE_SIZE);
                for (Column column : table.getColumns()) {
                    System.out.printf("COLUMN\t%s\t%s\t%s%n", name, column.getName(), column.getType());
                }
                for (Row row : table) {
                    if (name.equals("EvolvedPair")) {
                        RowIdImpl identity = (RowIdImpl) row.getId();
                        System.out.printf("LOCATION\t%s\t%d\t%d\t%d\t%d%n", name,
                            row.getInt("ID"), identity.getPageNumber(), identity.getRowNumber(),
                            nativeTable.getFormat().OFFSET_ROW_START);
                    }
                    System.out.printf("ROW\t%s\t%d", name, row.getInt("ID"));
                    for (Column column : table.getColumns()) {
                        if (!column.getName().equals("ID")) {
                            System.out.print("\t" + encoded(row.get(column.getName()), column.getType()));
                        }
                    }
                    System.out.println();
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl nativeIndex = (IndexImpl) index;
                    var data = nativeIndex.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s%n", name,
                        index.getName(), data.getIndexDataNumber(), data.getEntryCount(), data.getOwnedPageCount(),
                        nativeIndex.getUniqueEntryCount(), index.isUnique(), index.shouldIgnoreNulls());
                    for (Index.Column column : index.getColumns()) {
                        System.out.printf("KEY\t%s\t%s\t%s\t%s%n", name, index.getName(),
                            column.getName(), column.isAscending());
                    }
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE or oracle PATH");
        }
    }
}
