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
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;

/** Original inert public-builder trees and reference deletes; no stored SQL runs. */
public final class DeleteTreeFixture {
    private static final int ROWS = 1000;

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void inert(Database database) {
        database.setTimeZone(java.util.TimeZone.getTimeZone("UTC"));
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets are authorized");
        });
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh ACE16 output/profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table tree = new TableBuilder("Tree")
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
                .addIndex(new IndexBuilder("Compound")
                    .addColumns(false, "Part").addColumns("GroupKey"))
                .addIndex(new IndexBuilder("UniqueScore").addColumns("Score").setUnique())
                .toTable(database);
            for (int ordinal = 0; ordinal < ROWS; ordinal++) {
                int id = ordinal * 37 % ROWS + 1;
                Integer group = id % 17 == 0 ? null : id % 11 - 5;
                boolean longValue = id == 1 || id == 501 || id == 1000;
                String memo = longValue ? "中😀 preserved ".repeat(700) : "row=" + id;
                byte[] payload = new byte[longValue ? 12288 : 16];
                for (int position = 0; position < payload.length; position++) {
                    payload[position] = (byte) (id * 17 + position * 3);
                }
                tree.addRow(id, group, (short) (id % 5 - 2),
                    (id * 71 % ROWS) * 3 + 1, memo, payload);
            }
            Table unindexed = new TableBuilder("Unindexed")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .toTable(database);
            for (int id = 1; id <= 8; id++) {
                unindexed.addRow(id, id * 10);
            }
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("TextValue", DataType.TEXT).setLengthInUnits(32))
                .toTable(database).addRow(91, "unchanged tree sentinel 中");
        }
    }

    private static void delete(Path source, Path output, String selection) throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Reference output must be fresh");
        }
        Set<Integer> wanted = new HashSet<>();
        if (!selection.equals("all")) {
            for (String token : selection.split(",")) {
                if (!wanted.add(Integer.parseInt(token))) {
                    throw new IllegalArgumentException("Duplicate requested ID");
                }
            }
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            Table table = database.getTable("Tree");
            List<Row> selected = new ArrayList<>();
            for (Row row : table) {
                if (selection.equals("all") || wanted.contains(row.getInt("ID"))) {
                    selected.add(row);
                }
            }
            if (!selection.equals("all") && selected.size() != wanted.size()) {
                throw new IllegalArgumentException("Requested ID absent from source");
            }
            for (Row row : selected) {
                table.deleteRow(row);
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : List.of("Tree", "Unindexed", "Sentinel")) {
                Table table = database.getTable(name);
                TableImpl nativeTable = (TableImpl) table;
                var format = nativeTable.getFormat();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
                    table.getRowCount(), nativeTable.getTableDefPageNumber(), format.PAGE_SIZE,
                    format.OFFSET_NUM_ROWS, format.OFFSET_ROW_START,
                    format.OFFSET_INDEX_ENTRY_MASK, format.SIZE_INDEX_ENTRY_MASK);
                for (var column : table.getColumns()) {
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%d%n", name,
                        column.getName(), column.getType(), column.getColumnIndex());
                }
                for (Row row : table) {
                    int id = row.getInt("ID");
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d%n", name, id,
                        location.getPageNumber(), location.getRowNumber());
                    if (name.equals("Tree")) {
                        byte[] memo = row.getString("Memo").getBytes(StandardCharsets.UTF_8);
                        byte[] payload = row.getBytes("Payload");
                        System.out.printf("ROW\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%d\t%s%n",
                            name, id, row.get("GroupKey"), row.get("Part"), row.get("Score"),
                            memo.length, hash(memo), payload.length, hash(payload));
                    } else if (name.equals("Unindexed")) {
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id, row.get("Score"));
                    } else {
                        byte[] text = row.getString("TextValue").getBytes(StandardCharsets.UTF_8);
                        System.out.printf("ROW\t%s\t%d\t%s%n", name, id, hash(text));
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
                    for (var column : index.getColumns()) {
                        System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", name,
                            index.getName(), column.getName(), column.isAscending());
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
        } else if (arguments.length == 4 && arguments[0].equals("delete")) {
            delete(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE, delete SOURCE OUTPUT IDS/all, or oracle PATH");
        }
    }
}
