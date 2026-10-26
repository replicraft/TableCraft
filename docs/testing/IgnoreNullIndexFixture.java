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
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert public producer and read-only getter oracle; no reference SQL. */
public final class IgnoreNullIndexFixture {
    private static final int ROWS = 701;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets authorized");
        });
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Original output must be fresh");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table table = new TableBuilder("Ignored")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("KeyValue", DataType.LONG))
                .addColumn(new ColumnBuilder("UniqueValue", DataType.LONG))
                .addIndex(new IndexBuilder("IgnoredAsc").addColumns("KeyValue").setIgnoreNulls())
                .addIndex(new IndexBuilder("IgnoredAlias").addColumns("KeyValue").setIgnoreNulls())
                .addIndex(new IndexBuilder("IgnoredDesc")
                    .addColumns(false, "KeyValue").setIgnoreNulls())
                .addIndex(new IndexBuilder("UniqueIgnored")
                    .addColumns("UniqueValue").setIgnoreNulls().setUnique())
                .toTable(database);
            for (int ordinal = 0; ordinal < ROWS; ordinal++) {
                int id = ordinal * 37 % ROWS;
                Integer key = id % 11 == 0 ? null : id % 73 - 36;
                Integer unique = id % 13 == 0 ? null : id * 3 - 1050;
                table.addRow(id, key, unique);
            }
        }
    }

    private static String scalar(Object value) {
        return value == null ? "NULL" : value.toString();
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Table table = database.getTable("Ignored");
            TableImpl nativeTable = (TableImpl) table;
            System.out.printf("TABLE\t%s\t%d\t%d\t%d%n", table.getName(),
                table.getRowCount(), nativeTable.getTableDefPageNumber(),
                nativeTable.getFormat().PAGE_SIZE);
            for (Row row : table) {
                System.out.printf("ROW\t%d\t%s\t%s%n", row.getInt("ID"),
                    scalar(row.get("KeyValue")), scalar(row.get("UniqueValue")));
            }
            for (Index index : table.getIndexes()) {
                IndexImpl nativeIndex = (IndexImpl) index;
                var data = nativeIndex.getIndexData();
                System.out.printf("INDEX\t%s\t%d\t%d\t%d\t%d\t%s\t%s%n", index.getName(),
                    data.getIndexDataNumber(), data.getEntryCount(), data.getOwnedPageCount(),
                    nativeIndex.getUniqueEntryCount(), index.isUnique(), index.shouldIgnoreNulls());
                Cursor cursor = CursorBuilder.createCursor(index);
                Row row;
                while ((row = cursor.getNextRow()) != null) {
                    System.out.printf("ENTRY\t%s\t%d%n", index.getName(), row.getInt("ID"));
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
