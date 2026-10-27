import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.PageChannel;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.BitSet;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.UUID;

/** Original inert two-owner wide-index batch DELETE recipe and read-only getters. */
public final class BatchDeleteFixture {
    private static final int ROWS = 500;
    private static final int COMPONENTS = 10;
    private static final List<String> TABLES = List.of("Parents", "Children", "Sentinel");

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        if (!database.isEnforceForeignKeys()) {
            throw new AssertionError("Referential integrity must remain enabled");
        }
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked target is authorized");
        });
    }

    private static byte[] hash(byte[] bytes) throws Exception {
        return MessageDigest.getInstance("SHA-256").digest(bytes);
    }

    private static Table table(Database database, String name, boolean child) throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey());
        if (child) {
            builder.addColumn(new ColumnBuilder("ParentID", DataType.LONG));
            builder.addIndex(new IndexBuilder("ParentAsc").addColumns("ParentID"));
            builder.addIndex(new IndexBuilder("ParentAlias").addColumns("ParentID"));
        }
        String[] names = new String[COMPONENTS];
        for (int component = 0; component < COMPONENTS; component++) {
            names[component] = "G" + component;
            builder.addColumn(new ColumnBuilder(names[component], DataType.GUID));
        }
        builder.addIndex(new IndexBuilder("Wide").addColumns(names).setUnique());
        builder.addIndex(new IndexBuilder("WideAlias").addColumns(names).setUnique());
        return builder.toTable(database);
    }

    private static Object[] row(String table, int identity, boolean child) throws Exception {
        int firstGuid = child ? 2 : 1;
        Object[] row = new Object[firstGuid + COMPONENTS];
        row[0] = identity;
        if (child) {
            row[1] = identity;
        }
        for (int component = 0; component < COMPONENTS; component++) {
            String seed = "TableCraft original two-owner deletion/" + table + "/"
                + identity + "/" + component;
            ByteBuffer bits = ByteBuffer.wrap(hash(seed.getBytes(StandardCharsets.UTF_8)));
            row[firstGuid + component] = "{" + new UUID(bits.getLong(), bits.getLong()) + "}";
        }
        return row;
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected a fresh ACE16 original path/profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table parents = table(database, "Parents", false);
            Table children = table(database, "Children", true);
            new RelationshipBuilder(parents, children)
                .setName("Parents_Children")
                .addColumns("ID", "ParentID")
                .setReferentialIntegrity()
                .toRelationship(database);
            for (int ordinal = 0; ordinal < ROWS; ordinal++) {
                int identity = ordinal * 37 % ROWS + 1;
                parents.addRow(row("Parents", identity, false));
                children.addRow(row("Children", identity, true));
            }
            Table sentinel = new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .toTable(database);
            byte[] payload = new byte[12288];
            for (int position = 0; position < payload.length; position++) {
                payload[position] = (byte) (position * 13 + 71);
            }
            sentinel.addRow(91, "preserved 中😀 ".repeat(700), payload);
        }
    }

    private static void delete(Path source, Path output, String selection) throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Reference output must be fresh");
        }
        Set<Integer> selected = new HashSet<>();
        if (selection.equals("all")) {
            for (int identity = 1; identity <= ROWS; identity++) {
                selected.add(identity);
            }
        } else {
            for (String token : selection.split(",")) {
                if (!selected.add(Integer.parseInt(token))) {
                    throw new IllegalArgumentException("Duplicate selected ID");
                }
            }
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            // These are successive public writer calls, child before parent.
            // They are a final-state reference, not SQL action-timing evidence.
            for (String name : List.of("Children", "Parents")) {
                Table table = database.getTable(name);
                List<Row> rows = new ArrayList<>();
                for (Row row : table) {
                    if (selected.contains(row.getInt("ID"))) {
                        rows.add(row);
                    }
                }
                if (rows.size() != selected.size()) {
                    throw new AssertionError("Requested original IDs are incomplete");
                }
                for (Row row : rows) {
                    table.deleteRow(row);
                }
            }
        }
    }

    private static Object field(Object object, Class<?> owner, String name) throws Exception {
        Field member = owner.getDeclaredField(name);
        member.setAccessible(true);
        return member.get(object);
    }

    private static int number(Object object, Class<?> owner, String name) throws Exception {
        Method getter = owner.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(object)).intValue();
    }

    private static void map(String name, UsageMap map, int pages) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
            number(map, UsageMap.class, "getTablePageNumber"),
            number(map, UsageMap.class, "getRowStart"),
            number(map, UsageMap.class, "getRowEnd"),
            number(map, UsageMap.class, "getStartPage"),
            number(map, UsageMap.class, "getEndPage"), map.getPageCount());
        Method getter = UsageMap.class.getDeclaredMethod("getPageNumbers");
        getter.setAccessible(true);
        BitSet bits = (BitSet) getter.invoke(map);
        int base = number(map, UsageMap.class, "getStartPage");
        for (int page = 0; page < pages; page++) {
            boolean present = page >= base && bits.get(page - base);
            if (!name.equals("JackcessGlobalFree") && map.containsPageNumber(page) != present) {
                throw new AssertionError("Protected/public map membership differs");
            }
            if (present) {
                System.out.printf("MEMBER\t%s\t%d%n", name, page);
            }
        }
    }

    private static void maps(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            TableImpl first = (TableImpl) database.getTable("Parents");
            int width = first.getFormat().PAGE_SIZE;
            int pages = Math.toIntExact(Files.size(path) / width);
            System.out.printf("FILE\t%d\t%d%n", width, pages);
            UsageMap global = (UsageMap) field(first.getPageChannel(), PageChannel.class, "_globalUsageMap");
            map("JackcessGlobalFree", global, pages);
            for (String name : TABLES) {
                TableImpl table = (TableImpl) database.getTable(name);
                map(name + "Owned", (UsageMap) field(table, TableImpl.class, "_ownedPages"), pages);
                map(name + "FreeSpace", (UsageMap) field(table, TableImpl.class, "_freeSpacePages"), pages);
                Set<Integer> seen = new HashSet<>();
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    int entries = data.getEntryCount();
                    System.out.printf("INDEX\t%s/%s\t%d\t%d\t%d\t%d%n", name,
                        index.getName(), data.getIndexDataNumber(),
                        number(data, IndexData.class, "getRootPageNumber"),
                        data.getOwnedPageCount(), entries);
                    if (seen.add(data.getIndexDataNumber())) {
                        map("Index" + name + data.getIndexDataNumber() + "Owned",
                            (UsageMap) field(data, IndexData.class, "_ownedPages"), pages);
                    }
                }
            }
        }
    }

    private static void oracle(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            for (var relationship : database.getRelationships()) {
                System.out.printf("RELATION\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                    relationship.getName(), relationship.getFromTable().getName(),
                    relationship.getToTable().getName(), relationship.hasReferentialIntegrity(),
                    relationship.cascadeUpdates(), relationship.cascadeDeletes(),
                    relationship.cascadeNullOnDelete());
            }
            for (String name : TABLES) {
                TableImpl table = (TableImpl) database.getTable(name);
                var format = table.getFormat();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d%n", name,
                    table.getRowCount(), table.getTableDefPageNumber(), format.PAGE_SIZE,
                    format.OFFSET_NUM_ROWS, format.OFFSET_ROW_START,
                    format.OFFSET_INDEX_ENTRY_MASK, format.SIZE_INDEX_ENTRY_MASK);
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                    System.out.printf("COLUMN\t%s\t%s\t%s\t%d%n", name,
                        column.getName(), column.getType(), column.getColumnIndex());
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("LOCATION\t%s\t%d\t%d\t%d%n", name,
                        row.getInt("ID"), location.getPageNumber(), location.getRowNumber());
                    List<String> fields = new ArrayList<>();
                    fields.add("ROW");
                    fields.add(name);
                    fields.add(row.getInt("ID").toString());
                    if (name.equals("Sentinel")) {
                        fields.add(HexFormat.of().formatHex(hash(row.getString("Memo")
                            .getBytes(StandardCharsets.UTF_8))));
                        fields.add(HexFormat.of().formatHex(hash(row.getBytes("Payload"))));
                    } else {
                        if (name.equals("Children")) {
                            fields.add(row.getInt("ParentID").toString());
                        }
                        for (int component = 0; component < COMPONENTS; component++) {
                            fields.add(row.getString("G" + component));
                        }
                    }
                    System.out.println(String.join("\t", fields));
                }
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    int entries = data.getEntryCount();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s\t%s%n", name,
                        index.getName(), data.getIndexDataNumber(), index.getUniqueEntryCount(),
                        index.getUniqueEntryCountOffset(), entries, data.getOwnedPageCount(),
                        index.isUnique(), index.shouldIgnoreNulls());
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
        } else if (arguments.length == 2 && arguments[0].equals("maps")) {
            maps(Path.of(arguments[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE, delete SOURCE OUTPUT IDS/all, oracle PATH or maps PATH");
        }
    }
}
