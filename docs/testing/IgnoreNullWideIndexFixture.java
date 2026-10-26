import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert public producer; Long and exact Currency getters never use floating point. */
public final class IgnoreNullWideIndexFixture {
    private static final int ROWS = 701;
    private static final long[] KEYS = {
        Long.MIN_VALUE, Long.MIN_VALUE + 1, -9007199254740993L, -9007199254740992L,
        -10001L, -1L, 0L, 1L, 10001L, 9007199254740992L, 9007199254740993L,
        Long.MAX_VALUE - 1, Long.MAX_VALUE,
    };
    private static final long[] UNIQUE_EDGES = {
        Long.MIN_VALUE, Long.MIN_VALUE + 1, -9007199254740993L, -9007199254740992L,
        -1L, 0L, 1L, 9007199254740992L, 9007199254740993L,
        Long.MAX_VALUE - 1, Long.MAX_VALUE,
    };

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets authorized");
        });
    }

    private static Object stored(Long value, DataType type) {
        if (value == null) {
            return null;
        }
        return type == DataType.MONEY ? BigDecimal.valueOf(value, 4) : value;
    }

    private static void table(Database database, String name, DataType type) throws Exception {
        Table table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("KeyValue", type))
            .addColumn(new ColumnBuilder("UniqueValue", type))
            .addIndex(new IndexBuilder("IgnoredAsc").addColumns("KeyValue").setIgnoreNulls())
            .addIndex(new IndexBuilder("IgnoredAlias").addColumns("KeyValue").setIgnoreNulls())
            .addIndex(new IndexBuilder("IgnoredDesc")
                .addColumns(false, "KeyValue").setIgnoreNulls())
            .addIndex(new IndexBuilder("UniqueIgnored")
                .addColumns("UniqueValue").setIgnoreNulls().setUnique())
            .toTable(database);
        for (int ordinal = 0; ordinal < ROWS; ordinal++) {
            int id = ordinal * 37 % ROWS;
            Long key = id % 11 == 0 ? null : KEYS[id % KEYS.length];
            Long unique = id % 13 == 0 ? null
                : id <= UNIQUE_EDGES.length ? UNIQUE_EDGES[id - 1] : id + 10000L;
            table.addRow(id, stored(key, type), stored(unique, type));
        }
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Original output must be fresh");
        }
        Database.FileFormat format = Database.FileFormat.valueOf(profile);
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            inert(database);
            table(database, "IgnoredCurrency", DataType.MONEY);
            if (format == Database.FileFormat.V2016 || format == Database.FileFormat.V2019) {
                table(database, "IgnoredInt64", DataType.BIG_INT);
            }
        }
    }

    private static String exact(Object value, DataType type) {
        if (value == null) {
            return "NULL";
        }
        if (type == DataType.MONEY) {
            return Long.toString(((BigDecimal) value).setScale(4).unscaledValue().longValueExact());
        }
        if (!(value instanceof Long)) {
            throw new AssertionError("BIG_INT getter did not return Long");
        }
        return value.toString();
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : database.getTableNames()) {
                Table table = database.getTable(name);
                TableImpl nativeTable = (TableImpl) table;
                DataType type = table.getColumn("KeyValue").getType();
                if (type != DataType.MONEY && type != DataType.BIG_INT) {
                    throw new AssertionError("Unexpected key type");
                }
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%s%n", name, table.getRowCount(),
                    nativeTable.getTableDefPageNumber(), nativeTable.getFormat().PAGE_SIZE, type);
                for (Row row : table) {
                    Object key = row.get("KeyValue");
                    Object unique = row.get("UniqueValue");
                    System.out.printf("ROW\t%s\t%d\t%s\t%s%n", name, row.getInt("ID"),
                        exact(key, type), exact(unique, type));
                    if (key instanceof BigDecimal decimal) {
                        System.out.printf("MONEY\t%s\t%d\t%s\t%d\t%s%n", name, row.getInt("ID"),
                            decimal.toPlainString(), decimal.scale(), decimal.unscaledValue());
                    }
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl nativeIndex = (IndexImpl) index;
                    var data = nativeIndex.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s%n", name,
                        index.getName(), data.getIndexDataNumber(), data.getEntryCount(),
                        data.getOwnedPageCount(), nativeIndex.getUniqueEntryCount(),
                        index.isUnique(), index.shouldIgnoreNulls());
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
