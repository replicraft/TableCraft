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
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Relationship;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeSet;

/** Public V2016 SET NULL relationships; fixed single-parent delete references. */
public final class SetNullDeleteFixture {
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final int MAX_TABLES = 64;
    private static final int MAX_COLUMNS = 256;
    private static final int MAX_ROWS = 4096;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static String hex(String text) {
        return HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_16LE));
    }

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                return field.get(owner);
            } catch (NoSuchFieldException absent) {
                // Only the actual class hierarchy is searched.
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static int mapNumber(UsageMap map, String name) throws Exception {
        Method method = UsageMap.class.getDeclaredMethod(name);
        method.setAccessible(true);
        return ((Number) method.invoke(map)).intValue();
    }

    private static void map(String role, UsageMap map) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d\t%d\t%d%n", role,
            mapNumber(map, "getTablePageNumber"), mapNumber(map, "getRowStart"),
            mapNumber(map, "getRowEnd"), mapNumber(map, "getStartPage"),
            mapNumber(map, "getEndPage"), map.getPageCount());
        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        int count = 0;
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            if (++count > MAX_MAP_MEMBERS) {
                throw new AssertionError("Map member ceiling exceeded");
            }
            System.out.printf("MEMBER\t%s\t%d%n", role, page);
        }
    }

    private static void properties(String owner, PropertyMap values) throws Exception {
        for (PropertyMap.Property property : values) {
            Object value = property.getValue();
            System.out.printf("PROPERTY\t%s\t%s\t%d\t%s\t%s\t%s%n", owner,
                hex(property.getName()), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), value == null ? "null" : value.getClass().getName(),
                value == null ? "NULL" : hex(value.toString()));
        }
    }

    private static boolean mixed(String selected) {
        if (!selected.equals("basic") && !selected.equals("mixed")) {
            throw new IllegalArgumentException("Closed basic/mixed source required");
        }
        return selected.equals("mixed");
    }

    private static Table table(Database database, String name, boolean foreign) throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(false)
                .putProperty("Required", true));
        if (foreign) {
            builder.addColumn(new ColumnBuilder("ParentID", DataType.LONG)
                .putProperty("Required", false));
        }
        return builder.addColumn(new ColumnBuilder("Marker", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns(true, "ID").setPrimaryKey())
            .toTable(database);
    }

    private static void relationships(Database database, boolean expectedMixed) throws Exception {
        List<Relationship> relationships = database.getRelationships();
        if (relationships.size() != (expectedMixed ? 2 : 1)) {
            throw new AssertionError("Exact user relationship count required");
        }
        Set<String> names = new HashSet<>();
        for (Relationship relationship : relationships) {
            boolean nullAction = relationship.getName().equals("ParentChildNull");
            boolean cascadeAction = expectedMixed && relationship.getName().equals("ParentCascade");
            if ((!nullAction && !cascadeAction) || !names.add(relationship.getName())
                    || !relationship.hasReferentialIntegrity() || relationship.cascadeUpdates()
                    || relationship.cascadeNullOnDelete() != nullAction
                    || relationship.cascadeDeletes() != cascadeAction
                    || !relationship.getFromTable().getName().equals("Parents")
                    || !relationship.getToTable().getName().equals(nullAction ? "Children" : "CascadeChildren")
                    || relationship.getFromColumns().size() != 1
                    || relationship.getToColumns().size() != 1
                    || !relationship.getFromColumns().get(0).getName().equals("ID")
                    || !relationship.getToColumns().get(0).getName().equals("ParentID")) {
                throw new AssertionError("Exact enforced action declarations required");
            }
            System.out.printf("RELATIONSHIP\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                hex(relationship.getName()), hex(relationship.getFromTable().getName()),
                hex(relationship.getToTable().getName()), relationship.hasReferentialIntegrity(),
                relationship.cascadeUpdates(), relationship.cascadeDeletes(),
                relationship.cascadeNullOnDelete());
        }
    }

    private static void bounded(Path path) throws Exception {
        long length = Files.size(path);
        if (length <= 0 || length > MAX_FILE_BYTES || length % 4096 != 0) {
            throw new AssertionError("Bounded complete page image required");
        }
    }

    private static void create(Path path, String selected) throws Exception {
        boolean includeCascade = mixed(selected);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh original required");
        }
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ,
                StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.V2016).setAutoSync(false).create()) {
            inert(database);
            Table parents = table(database, "Parents", false);
            parents.addRow(1, 1001);
            parents.addRow(2, 1002);
            parents.addRow(3, 1003);
            Table children = table(database, "Children", true);
            children.addRow(10, 1, 101);
            children.addRow(11, 1, 102);
            children.addRow(12, 2, 103);
            children.addRow(13, null, 104);
            children.addRow(14, 3, 105);
            new RelationshipBuilder("Parents", "Children")
                .addColumns("ID", "ParentID").setName("ParentChildNull")
                .setReferentialIntegrity().setCascadeNullOnDelete().toRelationship(database);
            if (includeCascade) {
                Table cascade = table(database, "CascadeChildren", true);
                cascade.addRow(20, 1, 201);
                cascade.addRow(21, 2, 202);
                cascade.addRow(22, null, 203);
                new RelationshipBuilder("Parents", "CascadeChildren")
                    .addColumns("ID", "ParentID").setName("ParentCascade")
                    .setReferentialIntegrity().setCascadeDeletes().toRelationship(database);
            }
            table(database, "Sentinel", false).addRow(91, 12345);
            relationships(database, includeCascade);
            database.flush();
        }
        bounded(path);
        System.out.printf("CREATED\tV2016-%s%n", selected);
    }

    private static void delete(Path source, Path output, String selected) throws Exception {
        boolean includeCascade = mixed(selected);
        bounded(source);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Fresh reference copy required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel)
                .setAutoSync(false).open()) {
            inert(database);
            relationships(database, includeCascade);
            Table parents = database.getTable("Parents");
            if (parents.getRowCount() != 3 || database.getTable("Children").getRowCount() != 5
                    || (includeCascade && database.getTable("CascadeChildren").getRowCount() != 3)) {
                throw new AssertionError("Unchanged complete original required");
            }
            Cursor cursor = CursorBuilder.createCursor(parents.getIndex("PrimaryID"));
            if (!cursor.findFirstRow(parents.getColumn("ID"), 1)) {
                throw new AssertionError("Exact original parent required");
            }
            cursor.deleteCurrentRow();
            if (parents.getRowCount() != 2 || database.getTable("Children").getRowCount() != 5
                    || (includeCascade && database.getTable("CascadeChildren").getRowCount() != 2)) {
                throw new AssertionError("Complete public action counts required");
            }
            database.flush();
        }
        bounded(output);
        System.out.printf("DELETED\tV2016-%s\tParents\t1%n", selected);
    }

    private static void inspect(Path path, String selected) throws Exception {
        bounded(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Set<String> users = database.getTableNames();
            boolean includeCascade = mixed(selected);
            Set<String> expected = includeCascade
                ? Set.of("Parents", "Children", "CascadeChildren", "Sentinel")
                : Set.of("Parents", "Children", "Sentinel");
            if (!users.equals(expected)) {
                throw new AssertionError("Exactly the frozen user tables required");
            }
            relationships(database, includeCascade);
            TreeSet<String> names = new TreeSet<>(users);
            names.addAll(database.getSystemTableNames());
            if (names.size() > MAX_TABLES) {
                throw new AssertionError("Catalog ceiling exceeded");
            }
            boolean global = false;
            for (String name : names) {
                TableImpl table = (TableImpl) (users.contains(name)
                    ? database.getTable(name) : database.getSystemTable(name));
                String owner = "Table:" + table.getTableDefPageNumber();
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", hex(name),
                    table.getTableDefPageNumber(), table.getRowCount(), table.getColumnCount(),
                    table.getMaxColumnCount());
                if (table.getColumnCount() > MAX_COLUMNS) {
                    throw new AssertionError("Column ceiling exceeded");
                }
                if (!global) {
                    map("GlobalFree", (UsageMap) field(table.getPageChannel(), "_globalUsageMap"));
                    global = true;
                }
                map(owner + ":Owned", (UsageMap) field(table, "_ownedPages"));
                map(owner + ":Hint", (UsageMap) field(table, "_freeSpacePages"));
                properties(owner, table.getProperties());
                for (Column column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%d%n",
                        hex(name), hex(column.getName()), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                        Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                        column.isAutoNumber(), column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex());
                    properties(owner + ":Column:" + nativeColumn.getColumnNumber(), column.getProperties());
                    if (column.getType() == DataType.MEMO || column.getType() == DataType.OLE) {
                        String role = owner + ":Column:" + nativeColumn.getColumnNumber();
                        Object holder = field(column, "_lvalBufferH");
                        if (holder == null) {
                            System.out.printf("MAP_ABSENT\t%s\tNULL%n", role);
                        } else if (holder.getClass().getName().equals(
                                "com.healthmarketscience.jackcess.impl.LongValueColumnImpl$LegacyLongValueBufferHolder")) {
                            System.out.printf("MAP_ABSENT\t%s\tLEGACY%n", role);
                        } else {
                            if (!holder.getClass().getName().equals(
                                    "com.healthmarketscience.jackcess.impl.LongValueColumnImpl$UmapLongValueBufferHolder")) {
                                throw new AssertionError("Unknown long-value holder implementation");
                            }
                            map(role + ":Owned", (UsageMap) field(holder, "_ownedPages"));
                            map(role + ":Hint", (UsageMap) field(holder, "_freeSpacePages"));
                        }
                    }
                }
                Set<Integer> physical = new HashSet<>();
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    Method root = IndexData.class.getDeclaredMethod("getRootPageNumber");
                    root.setAccessible(true);
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%s%n",
                        hex(name), hex(index.getName()), data.getIndexDataNumber(), root.invoke(data),
                        data.getEntryCount(), data.getUniqueEntryCount(), data.getUniqueEntryCountOffset(),
                        Byte.toUnsignedInt(data.getIndexFlags()), index.isPrimaryKey());
                    for (Index.Column column : index.getColumns()) {
                        System.out.printf("KEY_COLUMN\t%s\t%s\t%s\t%s%n", hex(name), hex(index.getName()),
                            hex(column.getName()), column.isAscending());
                    }
                    if (physical.add(data.getIndexDataNumber())) {
                        map(owner + ":IndexData:" + data.getIndexDataNumber(), (UsageMap) field(data, "_ownedPages"));
                    }
                    if (users.contains(name)) {
                        Cursor cursor = CursorBuilder.createCursor(index);
                        Row row;
                        while ((row = cursor.getNextRow()) != null) {
                            RowIdImpl id = (RowIdImpl) row.getId();
                            System.out.printf("ENTRY\t%s\t%s\t%d\t%d\t%d%n", hex(name), hex(index.getName()),
                                row.getInt("ID"), id.getPageNumber(), id.getRowNumber());
                        }
                    }
                }
                if (!users.contains(name)) {
                    continue;
                }
                if (table.getRowCount() > MAX_ROWS) {
                    throw new AssertionError("User row ceiling exceeded");
                }
                int count = 0;
                for (Row row : table) {
                    int identity = row.getInt("ID");
                    RowIdImpl logical = (RowIdImpl) row.getId();
                    var state = table.createRowState();
                    ByteBuffer body = TableImpl.positionAtRowData(state, logical);
                    RowIdImpl target = state.getFinalRowId();
                    byte[] bytes = new byte[body.remaining()];
                    body.get(bytes);
                    System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d\t%d\t%s%n", hex(name), identity,
                        logical.getPageNumber(), logical.getRowNumber(), target.getPageNumber(), target.getRowNumber(),
                        HexFormat.of().formatHex(bytes));
                    for (Column column : table.getColumns()) {
                        Object value = row.get(column.getName());
                        String tag;
                        String encoded;
                        if (value == null) {
                            tag = "NULL";
                            encoded = "NULL";
                        } else if (value instanceof byte[] binary) {
                            tag = "BINARY";
                            encoded = HexFormat.of().formatHex(binary);
                        } else if (value instanceof String text) {
                            tag = "TEXT";
                            encoded = hex(text);
                        } else if (value instanceof Integer integer) {
                            tag = "INT32";
                            encoded = integer.toString();
                        } else {
                            throw new AssertionError("Unexpected captured user carrier");
                        }
                        System.out.printf("VALUE\t%s\t%d\t%s\t%s\t%s\t%s%n", hex(name), identity,
                            hex(column.getName()), value == null ? "null" : value.getClass().getName(), tag, encoded);
                    }
                    count++;
                }
                if (count != table.getRowCount()) {
                    throw new AssertionError("Complete user getter required");
                }
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("create")) {
            create(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 4 && arguments[0].equals("delete")) {
            delete(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3]);
        } else if (arguments.length == 3 && arguments[0].equals("inspect")) {
            inspect(Path.of(arguments[1]), arguments[2]);
        } else {
            throw new IllegalArgumentException("Closed create/delete/inspect arguments required");
        }
    }
}
