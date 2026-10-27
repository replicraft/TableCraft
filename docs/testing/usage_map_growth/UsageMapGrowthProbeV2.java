import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
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
import java.security.MessageDigest;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeSet;

/** Repository-authored finite producer and inert read-only map/getter probe. */
public final class UsageMapGrowthProbeV2 {
    private static final String SELECTED = "GrowthKeys";
    private static final String PADDING = "Padding";
    private static final String ID = "ID";
    private static final int TARGET_ROWS = 900;
    private static final int MARKER_FACTOR = 17;
    private static final int PADDING_ROWS = 700;
    private static final int PADDING_COLUMNS = 7;
    private static final int PADDING_UNITS = 255;
    private static final int MAX_MAP_MEMBERS = 32736;
    private static final int MAX_CATALOG_TABLES = 64;
    private static final long MAX_FILE_BYTES = 8L * 1024 * 1024;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        database.setReuseFreePages(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked database is authorized");
        });
    }

    private static Object field(Object owner, String name) throws Exception {
        for (Class<?> type = owner.getClass(); type != null; type = type.getSuperclass()) {
            try {
                Field member = type.getDeclaredField(name);
                member.setAccessible(true);
                return member.get(owner);
            } catch (NoSuchFieldException missing) {
                // Continue only through the actual object's class hierarchy.
            }
        }

        throw new NoSuchFieldException(name);
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void bounded(Path path) throws Exception {
        if (Files.size(path) > MAX_FILE_BYTES) {
            throw new IllegalArgumentException("Frozen file ceiling exceeded");
        }
    }

    private static void create(Path path, String profile) throws Exception {
        if (Files.exists(path) || !Set.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Fresh approved profile required");
        }

        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.CREATE_NEW, StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel)
                .setFileFormat(Database.FileFormat.valueOf(profile)).setAutoSync(false).create()) {
            inert(database);
            new TableBuilder(SELECTED)
                .addColumn(new ColumnBuilder(ID, DataType.LONG).putProperty("Required", true))
                .addColumn(new ColumnBuilder("KeyValue", DataType.LONG).putProperty("Required", true))
                .addColumn(new ColumnBuilder("Marker", DataType.LONG))
                .addIndex(new IndexBuilder("PrimaryID").addColumns(ID).setPrimaryKey())
                .addIndex(new IndexBuilder("Ascending").addColumns("KeyValue").setRequired())
                .addIndex(new IndexBuilder("Twin").addColumns("KeyValue").setRequired())
                .toTable(database);
            TableBuilder padding = new TableBuilder(PADDING)
                .addColumn(new ColumnBuilder(ID, DataType.LONG));
            for (int index = 0; index < PADDING_COLUMNS; index++) {
                padding.addColumn(new ColumnBuilder("Payload" + index, DataType.TEXT)
                    .setLengthInUnits(PADDING_UNITS).setCompressedUnicode(false));
            }
            padding.toTable(database);

            database.flush();
        }

        bounded(path);
    }

    private static void fill(Path path) throws Exception {
        bounded(path);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            TableImpl selected = (TableImpl) database.getTable(SELECTED);
            if (selected.getRowCount() != 0 || database.getTable(PADDING).getRowCount() != 0) {
                throw new IllegalArgumentException("Fresh empty source phase required");
            }

            for (int identity = 1; identity <= TARGET_ROWS; identity++) {
                selected.addRow(identity, 0, identity * MARKER_FACTOR);
            }
            database.flush();
        }

        bounded(path);
    }

    private static void pad(Path path) throws Exception {
        bounded(path);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(path,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(path).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            TableImpl selected = (TableImpl) database.getTable(SELECTED);
            TableImpl padding = (TableImpl) database.getTable(PADDING);
            if (selected.getRowCount() != TARGET_ROWS || padding.getRowCount() != 0) {
                throw new IllegalArgumentException("Wrong frozen construction phase");
            }

            String value = "Ā".repeat(PADDING_UNITS);
            for (int identity = 1; identity <= PADDING_ROWS; identity++) {
                Object[] row = new Object[PADDING_COLUMNS + 1];
                row[0] = identity;
                for (int index = 1; index < row.length; index++) {
                    row[index] = value;
                }
                padding.addRow(row);
            }
            database.flush();
        }

        bounded(path);
    }

    private static void reference(Path source, Path output) throws Exception {
        bounded(source);
        if (Files.exists(output) || source.toRealPath().equals(output.toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Fresh distinct reference required");
        }
        Files.copy(source, output);
        try (BoundedFileChannel channel = new BoundedFileChannel(FileChannel.open(output,
                StandardOpenOption.READ, StandardOpenOption.WRITE), MAX_FILE_BYTES);
             Database database = new DatabaseBuilder(output).setChannel(channel).setAutoSync(false).open()) {
            inert(database);
            TableImpl table = (TableImpl) database.getTable(SELECTED);
            if (table.getRowCount() != TARGET_ROWS || database.getTable(PADDING).getRowCount() != PADDING_ROWS) {
                throw new IllegalArgumentException("Frozen source population changed");
            }

            Cursor cursor = CursorBuilder.createCursor(table);
            boolean updated = false;
            while (cursor.moveToNextRow()) {
                Row row = cursor.getCurrentRow();
                if (row.getInt(ID) == 1) {
                    if (updated || row.getInt("KeyValue") != 0 || row.getInt("Marker") != MARKER_FACTOR) {
                        throw new IllegalArgumentException("Exact selected row precondition failed");
                    }
                    Map<String, Object> replacement = new HashMap<>();
                    replacement.put("KeyValue", Integer.MAX_VALUE);
                    cursor.updateCurrentRowFromMap(replacement);
                    updated = true;
                }
            }
            if (!updated) {
                throw new IllegalArgumentException("Frozen selected row absent");
            }
            database.flush();
        }

        bounded(output);
    }

    private static int mapNumber(UsageMap map, String method) throws Exception {
        Method getter = UsageMap.class.getDeclaredMethod(method);
        getter.setAccessible(true);
        return ((Number) getter.invoke(map)).intValue();
    }

    private static void map(String role, UsageMap map) throws Exception {
        System.out.printf("MAP\t%s\t%d\t%d\t%d\t%d%n", role,
            mapNumber(map, "getTablePageNumber"), mapNumber(map, "getRowStart"),
            mapNumber(map, "getRowEnd"), map.getPageCount());

        UsageMap.PageCursor cursor = map.cursor();
        cursor.beforeFirst();
        int count = 0;
        for (int page = cursor.getNextPage(); page >= 0; page = cursor.getNextPage()) {
            if (++count > MAX_MAP_MEMBERS) {
                throw new IllegalArgumentException("Map member capture ceiling exceeded");
            }
            System.out.printf("MEMBER\t%s\t%d%n", role, page);
        }
    }

    private static void properties(String owner, PropertyMap properties) throws Exception {
        for (PropertyMap.Property property : properties) {
            String value = String.valueOf(property.getValue());
            System.out.printf("PROPERTY\t%s\t%s\t%d\t%s\t%s%n", owner,
                property.getName(), Byte.toUnsignedInt(property.getType().getValue()),
                property.isDdl(), HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_16LE)));
        }
    }

    private static void catalog(Database database) throws Exception {
        Set<String> users = database.getTableNames();
        TreeSet<String> names = new TreeSet<>(users);
        names.addAll(database.getSystemTableNames());
        if (names.size() > MAX_CATALOG_TABLES) {
            throw new IllegalArgumentException("Catalog capture ceiling exceeded");
        }
        for (String name : names) {
            TableImpl table = (TableImpl) (users.contains(name)
                ? database.getTable(name) : database.getSystemTable(name));
            String owner = "Table:" + table.getTableDefPageNumber();
            System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", name,
                table.getTableDefPageNumber(), table.getRowCount(), table.getColumnCount(),
                table.getMaxColumnCount());
            map(owner + ":Owned", (UsageMap) field(table, "_ownedPages"));
            map(owner + ":Hint", (UsageMap) field(table, "_freeSpacePages"));

            for (Column column : table.getColumns()) {
                ColumnImpl nativeColumn = (ColumnImpl) column;
                System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%d%n",
                    name, column.getName(), column.getColumnIndex(), nativeColumn.getColumnNumber(),
                    Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                    column.isCompressedUnicode(), nativeColumn.getVarLenTableIndex());
                if (column.getType() == DataType.MEMO || column.getType() == DataType.OLE) {
                    Object holder = field(column, "_lvalBufferH");
                    String role = owner + ":Column:" + nativeColumn.getColumnNumber();
                    if (holder == null) {
                        System.out.printf("MAP_STATUS\t%s\tAbsentHolder%n", role);
                    } else {
                        map(role + ":Owned", (UsageMap) field(holder, "_ownedPages"));
                        map(role + ":Hint", (UsageMap) field(holder, "_freeSpacePages"));
                    }
                }
            }

            for (var index : table.getIndexes()) {
                IndexData data = index.getIndexData();
                Method root = IndexData.class.getDeclaredMethod("getRootPageNumber");
                root.setAccessible(true);
                System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d%n", name,
                    index.getName(), data.getIndexDataNumber(), root.invoke(data), data.getEntryCount());
                map(owner + ":Index:" + index.getName(), (UsageMap) field(data, "_ownedPages"));
                if (name.equals(SELECTED)) {
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ENTRY\t%s\t%d%n", index.getName(), row.getInt(ID));
                    }
                }
            }
        }
    }

    private static void getter(Database database, boolean changed) throws Exception {
        for (String tableName : List.of(SELECTED, PADDING)) {
            TableImpl table = (TableImpl) database.getTable(tableName);
            properties(tableName, table.getProperties());
            for (Column column : table.getColumns()) {
                properties(tableName + ":" + column.getName(), column.getProperties());
            }

            int count = 0;
            for (Row row : table) {
                int identity = row.getInt(ID);
                if (identity != count + 1) {
                    throw new AssertionError("Original insertion order changed");
                }
                RowIdImpl location = (RowIdImpl) row.getId();
                ByteBuffer buffer = TableImpl.positionAtRowData(table.createRowState(), location);
                byte[] bytes = new byte[buffer.remaining()];
                buffer.get(bytes);
                System.out.printf("ROW\t%s\t%d\t%d\t%d\t%d\t%s%n", tableName,
                    identity, location.getPageNumber(), location.getRowNumber(), bytes.length, hash(bytes));

                if (tableName.equals(SELECTED)) {
                    int key = row.getInt("KeyValue");
                    int expected = changed && identity == 1 ? Integer.MAX_VALUE : 0;
                    if (key != expected || row.getInt("Marker") != identity * MARKER_FACTOR) {
                        throw new AssertionError("Selected getter differs from frozen values");
                    }
                    System.out.printf("VALUE\t%d\t%d\t%d%n", identity, key, row.getInt("Marker"));
                    System.out.printf("RAWROW\t%d\t%s%n", identity, HexFormat.of().formatHex(bytes));
                } else {
                    for (int index = 0; index < PADDING_COLUMNS; index++) {
                        if (!"Ā".repeat(PADDING_UNITS).equals(row.getString("Payload" + index))) {
                            throw new AssertionError("Padding getter mismatch");
                        }
                    }
                }
                count++;
            }
            if (count != table.getRowCount()) {
                throw new AssertionError("Complete getter count mismatch");
            }
        }
        System.out.printf("POLICY\t%s\t%s\t%s\t%s%n", database.isEvaluateExpressions(),
            database.isEnforceForeignKeys(), database.isReuseFreePages(), database.getTimeZone().getID());
        if (database.isEvaluateExpressions() || !database.isEnforceForeignKeys()
                || database.isReuseFreePages() || !database.getTimeZone().getID().equals("UTC")) {
            throw new AssertionError("Inert operation policies changed");
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length < 2) {
            throw new IllegalArgumentException("Explicit operation and path required");
        }

        String mode = arguments[0];
        Path path = Path.of(arguments[1]);
        if (mode.equals("create") && arguments.length == 3) {
            create(path, arguments[2]);
        } else if (mode.equals("fill") && arguments.length == 2) {
            fill(path);
        } else if (mode.equals("pad") && arguments.length == 2) {
            pad(path);
        } else if (mode.equals("reference") && arguments.length == 3) {
            reference(path, Path.of(arguments[2]));
        } else if (mode.equals("inspect") && arguments.length == 3
                && Set.of("before", "after").contains(arguments[2])) {
            bounded(path);
            try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
                inert(database);
                getter(database, arguments[2].equals("after"));
                TableImpl selected = (TableImpl) database.getTable(SELECTED);
                map("GlobalFree", (UsageMap) field(selected.getPageChannel(), "_globalUsageMap"));
                catalog(database);
            }
        } else {
            throw new IllegalArgumentException("Operation outside frozen recipe");
        }
    }
}
