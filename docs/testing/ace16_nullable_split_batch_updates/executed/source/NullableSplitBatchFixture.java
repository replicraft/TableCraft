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
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeSet;

/** Fixed public original sources only; no UPDATE or preview operation is exposed. */
public final class NullableSplitBatchFixture {
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final int MAX_TABLES = 64;
    private static final int MAX_COLUMNS = 256;
    private static final int MAX_ROWS = 4096;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setAllowAutoNumberInsert(false);
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

    private static void bounded(Path path) throws Exception {
        long length = Files.size(path);
        if (length <= 0 || length > MAX_FILE_BYTES || length % 4096 != 0) {
            throw new AssertionError("Bounded complete page image required");
        }
    }

    private static Database.FileFormat profile(String selected) {
        if (!Set.of("V2016", "V2019").contains(selected)) {
            throw new IllegalArgumentException("Exactly the two ACE 16 layouts required");
        }
        return Database.FileFormat.valueOf(selected);
    }

    private static int last(TableImpl table, String name) throws Exception {
        Method getter = TableImpl.class.getDeclaredMethod(name);
        getter.setAccessible(true);
        return ((Number) getter.invoke(table)).intValue();
    }

    private static int signed(int ordinal) {
        return (ordinal % 2 == 0)
            ? Integer.MIN_VALUE + ordinal * 17
            : Integer.MAX_VALUE - ordinal * 17;
    }

    private static boolean direction(String selected) {
        if (!Set.of("ASC", "DESC").contains(selected)) {
            throw new IllegalArgumentException("Exactly ASC or DESC required");
        }
        return selected.equals("ASC");
    }

    private static IndexBuilder index(String name, boolean pair, boolean ascending,
            boolean unique) {
        IndexBuilder result = new IndexBuilder(name);
        if (pair) {
            result.addColumns(!ascending, "Score").addColumns(ascending, "Group");
            result.setIgnoreNulls();
        } else {
            result.addColumns(ascending, "Group");
        }
        if (unique) {
            result.setUnique();
        }
        return result;
    }

    private static void table(Database database, String name, boolean ascending,
            boolean unique) throws Exception {
        Table table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Group", DataType.LONG))
            .addColumn(new ColumnBuilder("Score", DataType.LONG))
            .addIndex(index("LeftOrder", false, ascending, unique))
            .addIndex(index("LeftAlias", false, ascending, unique))
            .addIndex(index("PairOrder", true, ascending, unique))
            .addIndex(index("PairAlias", true, ascending, unique))
            .toTable(database);
        for (int ordinal = 0; ordinal < 500; ordinal++) {
            Integer group = ordinal < 300 ? null : signed(ordinal);
            table.addRow(ordinal + 1, group, null);
        }
    }

    private static void create(Path path, String layout, String selected) throws Exception {
        Database.FileFormat format = profile(layout);
        boolean ascending = direction(selected);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh original required");
        }
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ,
                StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(format).setAutoSync(false).create()) {
            inert(database);
            table(database, "Items", ascending, false);
            table(database, "UniqueItems", ascending, true);
            new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .toTable(database).addRow(91, -12345);
        }
        bounded(path);
        System.out.printf("CREATED\t%s\t%s%n", layout, selected);
    }

    private static void inspect(Path path, String layout) throws Exception {
        profile(layout);
        bounded(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Set<String> users = database.getTableNames();
            if (!users.equals(Set.of("Items", "UniqueItems", "Sentinel"))) {
                throw new AssertionError("Exactly the frozen three user tables required");
            }
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
                System.out.printf("AUTONUMBER\t%s\t%d\t%d\t%s%n", hex(name),
                    last(table, "getLastLongAutoNumber"),
                    last(table, "getLastComplexTypeAutoNumber"), table.isAllowAutoNumberInsert());
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
                    if (users.contains(name) && !name.equals("Sentinel")) {
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
        if (arguments.length == 4 && arguments[0].equals("create")) {
            create(Path.of(arguments[1]), arguments[2], arguments[3]);
        } else if (arguments.length == 3 && arguments[0].equals("inspect")) {
            inspect(Path.of(arguments[1]), arguments[2]);
        } else {
            throw new IllegalArgumentException("Closed source create/inspect arguments required");
        }
    }
}
