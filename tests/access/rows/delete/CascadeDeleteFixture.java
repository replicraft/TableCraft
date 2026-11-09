import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import java.lang.reflect.Method;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Original small cascade graph producer; writer probes operate on fresh copies only. */
public final class CascadeDeleteFixture {
    private static final int DECIMAL_PRECISION = 28;
    private static final int DECIMAL_SCALE = 4;

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

    private static Table plain(Database database, String name, String... foreignColumns)
            throws Exception {
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey());
        for (String column : foreignColumns) {
            builder.addColumn(new ColumnBuilder(column, DataType.LONG));
            builder.addIndex(new IndexBuilder(column + "Asc").addColumns(column));
            builder.addIndex(new IndexBuilder(column + "Alias").addColumns(column));
        }
        return builder.toTable(database);
    }

    private static void rows(Table table, Object[][] rows) throws Exception {
        for (Object[] row : rows) {
            table.addRow(row);
        }
    }

    private static void relation(Database database, Table parent, Table child,
            String parentColumn, String childColumn) throws Exception {
        new RelationshipBuilder(parent, child)
            .setName(parent.getName() + "_" + child.getName())
            .addColumns(parentColumn, childColumn)
            .setReferentialIntegrity()
            .setCascadeDeletes()
            .toRelationship(database);
    }

    private static void chains(Database database) throws Exception {
        Table root = plain(database, "ChainRoot");
        Table branch = plain(database, "ChainBranch", "RootID");
        Table leaf = plain(database, "ChainLeaf", "BranchID");
        rows(root, new Object[][] {{1}, {2}, {3}, {99}});
        rows(branch, new Object[][] {
            {11, 1}, {12, 1}, {21, 2}, {31, 3}, {91, 99}, {92, null}
        });
        rows(leaf, new Object[][] {
            {111, 11}, {112, 12}, {211, 21}, {311, 31}, {911, 91}, {912, null}
        });
        relation(database, root, branch, "ID", "RootID");
        relation(database, branch, leaf, "ID", "BranchID");
    }

    private static void diamond(Database database) throws Exception {
        Table root = plain(database, "DiamondRoot");
        Table left = plain(database, "DiamondLeft", "RootID");
        Table right = plain(database, "DiamondRight", "RootID");
        Table shared = plain(database, "DiamondLeaf", "LeftID", "RightID");
        rows(root, new Object[][] {{1}, {2}, {99}});
        rows(left, new Object[][] {{11, 1}, {12, 1}, {21, 2}, {91, 99}, {92, null}});
        rows(right, new Object[][] {{101, 1}, {201, 2}, {202, 2}, {901, 99}, {902, null}});
        rows(shared, new Object[][] {
            {1001, 11, 101}, {1002, 11, 201}, {1003, 21, 101},
            {1004, 21, 202}, {1999, 91, 901}, {1998, null, null}
        });
        relation(database, root, left, "ID", "RootID");
        relation(database, root, right, "ID", "RootID");
        relation(database, left, shared, "ID", "LeftID");
        relation(database, right, shared, "ID", "RightID");
    }

    private static ColumnBuilder keyColumn(DataType type) {
        ColumnBuilder column = new ColumnBuilder("KeyValue", type);
        if (type == DataType.NUMERIC) {
            column.setPrecision(DECIMAL_PRECISION).setScale(DECIMAL_SCALE);
        }
        return column;
    }

    private static void pair(Database database, String name, DataType type, Object[] keys)
            throws Exception {
        Table parent = new TableBuilder(name + "Parents")
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(keyColumn(type))
            .addIndex(new IndexBuilder("PrimaryKey").addColumns("KeyValue").setPrimaryKey())
            .toTable(database);
        Table child = new TableBuilder(name + "Children")
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(keyColumn(type))
            .addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey())
            .addIndex(new IndexBuilder("KeyAsc").addColumns("KeyValue"))
            .addIndex(new IndexBuilder("KeyAlias").addColumns("KeyValue"))
            .toTable(database);
        rows(parent, new Object[][] {{1, keys[0]}, {2, keys[1]}, {3, keys[2]}, {99, keys[3]}});
        rows(child, new Object[][] {
            {11, keys[0]}, {12, keys[0]}, {21, keys[1]},
            {31, keys[2]}, {91, keys[3]}, {92, null}
        });
        relation(database, parent, child, "KeyValue", "KeyValue");
    }

    private static void typedPairs(Database database) throws Exception {
        pair(database, "Byte", DataType.BYTE, new Object[] {(byte) 0, (byte) 1, (byte) 255, (byte) 99});
        pair(database, "I16", DataType.INT, new Object[] {Short.MIN_VALUE, (short) 0, Short.MAX_VALUE, (short) 99});
        pair(database, "I32", DataType.LONG, new Object[] {Integer.MIN_VALUE, -1, Integer.MAX_VALUE, 99});
        pair(database, "I64", DataType.BIG_INT,
            new Object[] {Long.MIN_VALUE, -9007199254740993L, Long.MAX_VALUE, 99L});
        pair(database, "Currency", DataType.MONEY, new Object[] {
            new BigDecimal("-922337203685477.5808"), new BigDecimal("0.0000"),
            new BigDecimal("922337203685477.5807"), new BigDecimal("99.0000")
        });
        pair(database, "Guid", DataType.GUID, new Object[] {
            "{00000000-0000-0000-0000-000000000000}",
            "{00112233-4455-6677-8899-AABBCCDDEEFF}",
            "{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}",
            "{99999999-0000-0000-0000-000000000099}"
        });
        BigDecimal maximum = new BigDecimal(new BigInteger("9999999999999999999999999999"), DECIMAL_SCALE);
        pair(database, "Decimal", DataType.NUMERIC,
            new Object[] {maximum.negate(), new BigDecimal("0.0000"), maximum, new BigDecimal("99.0000")});
    }

    private static void selfReferences(Database database) throws Exception {
        Table tree = plain(database, "SelfTree", "ParentID");
        Table loop = plain(database, "SelfLoop", "ParentID");
        Table cycle = plain(database, "SelfCycle", "ParentID");
        rows(tree, new Object[][] {{1, null}, {2, 1}, {3, 1}, {4, 2}, {99, null}, {100, null}});
        rows(loop, new Object[][] {{1, 1}, {99, null}});
        rows(cycle, new Object[][] {{1, 2}, {2, 1}, {99, null}});
        relation(database, tree, tree, "ID", "ParentID");
        relation(database, loop, loop, "ID", "ParentID");
        relation(database, cycle, cycle, "ID", "ParentID");
        Table first = plain(database, "CycleA", "OtherID");
        Table second = plain(database, "CycleB", "OtherID");
        rows(first, new Object[][] {{1, 1}, {99, 99}});
        rows(second, new Object[][] {{1, 1}, {99, 99}});
        relation(database, first, second, "ID", "OtherID");
        relation(database, second, first, "ID", "OtherID");
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected a fresh ACE16 original path/profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            chains(database);
            diamond(database);
            typedPairs(database);
            selfReferences(database);
            Table sentinel = new TableBuilder("Sentinel")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Payload", DataType.OLE))
                .toTable(database);
            byte[] payload = new byte[12288];
            for (int index = 0; index < payload.length; index++) {
                payload[index] = (byte) (13 * index + 71);
            }
            sentinel.addRow(91, "preserved 中😀 ".repeat(700), payload);
        }
    }

    private static String value(Object value, DataType type) throws Exception {
        if (value == null) {
            return "NULL";
        }
        if (value instanceof BigDecimal decimal) {
            return "Decimal:" + decimal.unscaledValue() + ":" + decimal.scale();
        }
        if (type == DataType.MEMO) {
            byte[] encoded = value.toString().getBytes(StandardCharsets.UTF_8);
            return "Memo:" + java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(encoded));
        }
        if (value instanceof byte[] bytes) {
            return "Binary:" + bytes.length + ":"
                + java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        }
        return value.getClass().getSimpleName() + ":" + value;
    }

    private static int root(IndexData data) throws Exception {
        Method getter = IndexData.class.getDeclaredMethod("getRootPageNumber");
        getter.setAccessible(true);
        return ((Number) getter.invoke(data)).intValue();
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
                for (int index = 0; index < relationship.getFromColumns().size(); index++) {
                    System.out.printf("RELATION_COLUMN\t%s\t%d\t%s\t%s%n", relationship.getName(), index,
                        relationship.getFromColumns().get(index).getName(),
                        relationship.getToColumns().get(index).getName());
                }
            }
            for (String name : database.getTableNames()) {
                TableImpl table = (TableImpl) database.getTable(name);
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", name,
                    table.getRowCount(), table.getTableDefPageNumber(),
                    table.getColumnCount(), table.getMaxColumnCount());
                if (table.getProperties().iterator().hasNext()) {
                    throw new AssertionError("Unexpected table properties");
                }
                for (var column : table.getColumns()) {
                    if (column.getProperties().iterator().hasNext()) {
                        throw new AssertionError("Unexpected column properties");
                    }
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%s%n",
                        name, column.getName(), column.getColumnIndex(),
                        Byte.toUnsignedInt(column.getType().getValue()), column.getLength(),
                        column.getType() == DataType.NUMERIC ? column.getPrecision() : 0,
                        column.getType() == DataType.NUMERIC ? column.getScale() : 0,
                        column.isAutoNumber());
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    List<String> fields = new ArrayList<>();
                    fields.add("ROW");
                    fields.add(name);
                    fields.add(row.getInt("ID").toString());
                    fields.add(Integer.toString(location.getPageNumber()));
                    fields.add(Integer.toString(location.getRowNumber()));
                    for (var column : table.getColumns()) {
                        fields.add(value(row.get(column.getName()), column.getType()));
                    }
                    System.out.println(String.join("\t", fields));
                }
                for (var index : table.getIndexes()) {
                    IndexData data = index.getIndexData();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(), root(data),
                        data.getEntryCount(), data.getOwnedPageCount(), index.isPrimaryKey(),
                        index.isUnique(), index.isRequired(), index.shouldIgnoreNulls());
                    for (var column : index.getColumns()) {
                        System.out.printf("INDEX_COLUMN\t%s\t%s\t%s\t%s%n", name,
                            index.getName(), column.getName(), column.isAscending());
                    }
                }
            }
        }
    }

    private static void delete(Path source, Path output, String tableName, String selection)
            throws Exception {
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Action output must be fresh");
        }
        Map<Integer, Boolean> identifiers = new LinkedHashMap<>();
        for (String token : selection.split(",")) {
            if (identifiers.put(Integer.parseInt(token), Boolean.TRUE) != null) {
                throw new IllegalArgumentException("Duplicate explicit root");
            }
        }
        Files.copy(source, output);
        try (Database database = new DatabaseBuilder(output).open()) {
            inert(database);
            Table table = database.getTable(tableName);
            Map<Integer, Row> rows = new LinkedHashMap<>();
            for (Row row : table) {
                if (identifiers.containsKey(row.getInt("ID"))) {
                    rows.put(row.getInt("ID"), row);
                }
            }
            if (rows.size() != identifiers.size()) {
                throw new IllegalArgumentException("Requested original root is absent");
            }
            for (int identity : identifiers.keySet()) {
                System.out.printf("ACTION_BEGIN\t%s\t%d%n", tableName, identity);
                System.out.flush();
                table.deleteRow(rows.get(identity));
                System.out.printf("ACTION_END\t%s\t%d%n", tableName, identity);
                System.out.flush();
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 2 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]));
        } else if (arguments.length == 5 && arguments[0].equals("delete")) {
            delete(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3], arguments[4]);
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE, oracle PATH, or delete SOURCE OUTPUT TABLE IDS");
        }
    }
}
