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
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** Original inert public-writer delete recipe and read-only metadata/getter oracle. */
public final class DeleteStorageFixture {
    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets are authorized");
        });
    }

    private static void generate(Path directory, String profile) throws Exception {
        Path original = directory.resolve("Delete-" + profile + "-before.accdb");
        if (Files.exists(original)) {
            throw new IllegalArgumentException("Original output must be fresh");
        }
        try (Database database = new DatabaseBuilder(original)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table indexed = new TableBuilder("Indexed")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("GroupKey", DataType.LONG))
                .addColumn(new ColumnBuilder("Part", DataType.INT))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
                .addIndex(new IndexBuilder("GroupAsc").addColumns("GroupKey"))
                .addIndex(new IndexBuilder("GroupTwin").addColumns("GroupKey"))
                .addIndex(new IndexBuilder("GroupDesc").addColumns(false, "GroupKey"))
                .addIndex(new IndexBuilder("GroupIgnoreNulls").addColumns("GroupKey").setIgnoreNulls())
                .addIndex(new IndexBuilder("Compound").addColumns("GroupKey", "Part"))
                .addIndex(new IndexBuilder("UniqueScore").addColumns("Score").setUnique())
                .toTable(database);
            Integer[] groups = {10, 10, null, null, -1, 20};
            for (int index = 0; index < groups.length; index++) {
                int id = index + 1;
                byte[] payload = new byte[8192];
                for (int position = 0; position < payload.length; position++) {
                    payload[position] = (byte) (id * 17 + position * 3);
                }
                indexed.addRow(id, groups[index], (short) (index % 2 + 1), id * 100,
                    "row=" + id + ":中😀:" + "m".repeat(4096), payload);
            }
            Table unindexed = new TableBuilder("Unindexed")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .toTable(database);
            for (int id = 1; id <= 3; id++) {
                unindexed.addRow(id, id * 10);
            }
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("TextValue", DataType.TEXT).setLengthInUnits(32))
                .toTable(database).addRow(91, "unchanged sentinel 中");
        }

        for (String operation : List.of("delete-one", "delete-null", "delete-all")) {
            Path copy = directory.resolve("Delete-" + profile + "-" + operation + ".accdb");
            Files.copy(original, copy);
            try (Database database = new DatabaseBuilder(copy).open()) {
                inert(database);
                for (String name : List.of("Indexed", "Unindexed")) {
                    Table table = database.getTable(name);
                    List<Row> selected = new ArrayList<>();
                    for (Row row : table) {
                        int id = row.getInt("ID");
                        if (operation.equals("delete-all")
                                || id == (operation.equals("delete-null") ? 3 : 2)) {
                            selected.add(row);
                        }
                    }
                    for (Row row : selected) {
                        table.deleteRow(row);
                    }
                }
            }
        }
    }

    private static String scalar(Object value) {
        return value == null ? "NULL" : value.toString();
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : List.of("Indexed", "Unindexed", "Sentinel")) {
                Table table = database.getTable(name);
                TableImpl nativeTable = (TableImpl) table;
                var format = nativeTable.getFormat();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
                    table.getRowCount(), nativeTable.getTableDefPageNumber(), format.PAGE_SIZE,
                    format.OFFSET_NUM_ROWS, format.OFFSET_ROW_START,
                    format.OFFSET_INDEX_ENTRY_MASK, format.SIZE_INDEX_ENTRY_MASK);
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    int id = row.getInt("ID");
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d%n", name, id,
                        location.getPageNumber(), location.getRowNumber());
                    if (name.equals("Indexed")) {
                        byte[] memo = row.getString("Memo").getBytes(StandardCharsets.UTF_8);
                        byte[] payload = row.getBytes("Payload");
                        System.out.printf("ROW\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%d\t%s%n",
                            name, id, scalar(row.get("GroupKey")), scalar(row.get("Part")),
                            scalar(row.get("Score")), memo.length, hash(memo), payload.length, hash(payload));
                    } else if (name.equals("Unindexed")) {
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id, scalar(row.get("Score")));
                    } else {
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id,
                            hash(row.getString("TextValue").getBytes(StandardCharsets.UTF_8)));
                    }
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl nativeIndex = (IndexImpl) index;
                    var data = nativeIndex.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(),
                        nativeIndex.getUniqueEntryCount(), nativeIndex.getUniqueEntryCountOffset(),
                        data.getEntryCount(), data.getOwnedPageCount(), index.isUnique(),
                        index.shouldIgnoreNulls());
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
            throw new IllegalArgumentException("Expected generate DIRECTORY PROFILE or oracle PATH");
        }
    }
}
