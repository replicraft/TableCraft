import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.UUID;

/** Original deterministic wide-key producer and disposable public deletes. */
public final class DeepDeleteFixture {
    private static final int ROWS = 500;
    private static final int COMPONENTS = 10;

    private static byte[] sha(byte[] bytes) throws Exception {
        return MessageDigest.getInstance("SHA-256").digest(bytes);
    }

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets are authorized");
        });
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh ACE16 path/profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            TableBuilder builder = new TableBuilder("Deep")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey());
            String[] names = new String[COMPONENTS];
            for (int component = 0; component < COMPONENTS; component++) {
                names[component] = "G" + component;
                builder.addColumn(new ColumnBuilder(names[component], DataType.GUID));
            }
            builder.addIndex(new IndexBuilder("Wide").addColumns(names).setUnique());
            builder.addIndex(new IndexBuilder("WideAlias").addColumns(names).setUnique());
            Table table = builder.toTable(database);
            for (int ordinal = 0; ordinal < ROWS; ordinal++) {
                int id = ordinal * 37 % ROWS + 1;
                Object[] row = new Object[COMPONENTS + 1];
                row[0] = id;
                for (int component = 0; component < COMPONENTS; component++) {
                    byte[] seed = ("TableCraft original deep deletion/" + id + "/" + component)
                        .getBytes(StandardCharsets.UTF_8);
                    ByteBuffer bits = ByteBuffer.wrap(sha(seed));
                    UUID guid = new UUID(bits.getLong(), bits.getLong());
                    row[component + 1] = "{" + guid + "}";
                }
                table.addRow(row);
            }
            Table preserved = new TableBuilder("Preserved")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .toTable(database);
            byte[] payload = new byte[12288];
            for (int position = 0; position < payload.length; position++) {
                payload[position] = (byte) (position * 13 + 71);
            }
            preserved.addRow(91, "preserved 中😀 ".repeat(700), payload);
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
            Table table = database.getTable("Deep");
            List<Row> selected = new ArrayList<>();
            for (Row row : table) {
                if (selection.equals("all") || wanted.contains(row.getInt("ID"))) {
                    selected.add(row);
                }
            }
            if (!selection.equals("all") && selected.size() != wanted.size()) {
                throw new IllegalArgumentException("Requested ID absent");
            }
            for (Row row : selected) {
                table.deleteRow(row);
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (String name : List.of("Deep", "Preserved")) {
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
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d%n", name, row.getInt("ID"),
                        location.getPageNumber(), location.getRowNumber());
                    List<String> fields = new ArrayList<>();
                    fields.add("ROW");
                    fields.add(name);
                    fields.add(row.getInt("ID").toString());
                    if (name.equals("Deep")) {
                        for (int component = 0; component < COMPONENTS; component++) {
                            fields.add(row.getString("G" + component));
                        }
                    } else {
                        fields.add(HexFormat.of().formatHex(sha(row.getString("Memo")
                            .getBytes(StandardCharsets.UTF_8))));
                        fields.add(HexFormat.of().formatHex(sha(row.getBytes("Payload"))));
                    }
                    System.out.println(String.join("\t", fields));
                }
                for (var index : nativeTable.getIndexes()) {
                    var data = index.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s\t%s%n", name,
                        index.getName(), data.getIndexDataNumber(), index.getUniqueEntryCount(),
                        index.getUniqueEntryCountOffset(), data.getEntryCount(),
                        data.getOwnedPageCount(), index.isUnique(), index.shouldIgnoreNulls());
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
