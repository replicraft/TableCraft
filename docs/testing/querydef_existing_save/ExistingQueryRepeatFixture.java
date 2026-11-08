import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
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
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.Map;

/** Fresh inert engineering inputs and explicit SDK metadata updates only. */
public final class ExistingQueryRepeatFixture {
    private static final int TARGET = Integer.MIN_VALUE + 7100;
    private static final int OTHER = TARGET + 1;
    private static final int WIDE = TARGET + 2;

    private static void require(boolean condition, String message) {
        if (!condition) {
            throw new AssertionError(message);
        }
    }

    private static String text(Object value) {
        return value == null ? "NULL" : value.toString().replace("\\", "\\\\")
                .replace("\t", "\\t").replace("\r", "\\r").replace("\n", "\\n");
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static byte[] order(int value) {
        return ByteBuffer.allocate(4).putInt(value).array();
    }

    private static void fragment(Table table, int id, byte attribute, String expression,
                                 Short flag, String first, String second, int ordinal) throws Exception {
        table.addRowFromMap(new QueryImpl.Row(null, attribute, expression, flag,
                null, first, second, id, order(ordinal)).toTableRow());
    }

    private static int tablesParent(Database database) throws Exception {
        for (Row row : database.getSystemTable("MSysObjects")) {
            if ("Tables".equals(row.getString("Name"))
                    && Short.valueOf((short) 3).equals(row.get("Type"))) {
                return row.getInt("Id");
            }
        }
        throw new AssertionError("missing Tables namespace");
    }

    private static void query(Database database, int id, String name, int columns,
                              boolean localSource, String literal) throws Exception {
        Map<String, Object> object = new HashMap<>();
        object.put("Id", id);
        object.put("ParentId", tablesParent(database));
        object.put("Name", name);
        object.put("Type", (short) 5);
        object.put("Flags", 0);
        database.getSystemTable("MSysObjects").addRowFromMap(object);
        Table fragments = database.getSystemTable("MSysQueries");
        fragment(fragments, id, (byte) 0, null, null, null, null, 1);
        fragment(fragments, id, (byte) 1, null, (short) 1, null, null, 1);
        fragment(fragments, id, (byte) 3, null, (short) 0, null, null, 1);
        if (localSource) {
            fragment(fragments, id, (byte) 5, null, null, "LocalData", "p", 1);
        }
        for (int column = 0; column < columns; column++) {
            String expression = literal == null ? Integer.toString(column) : literal;
            String alias = id == TARGET ? "Answer" : "Other" + column;
            fragment(fragments, id, (byte) 6, expression, null, alias, null, column + 1);
        }
        if (localSource) {
            fragment(fragments, id, (byte) 8, "[p].[ID] >= 2", null, null, null, 1);
        }
        fragment(fragments, id, (byte) 255, null, null, null, null, 1);
    }

    private static void create(Path path, boolean multiplePages) throws Exception {
        require(!Files.exists(path), "fresh source path required");
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.V2016).create()) {
            database.setEvaluateExpressions(false);
            database.setAllowAutoNumberInsert(true);
            Table local = new TableBuilder("LocalData")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Score", DataType.LONG))
                    .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
                    .toTable(database);
            local.addRow(1, 10);
            local.addRow(2, 20);
            if (multiplePages) {
                query(database, WIDE, "ManyLocalColumns", 96, true, null);
            }
            query(database, OTHER, "OtherConstant", 1, false, "99");
            // Target comes last, avoiding authority based on first Column row.
            query(database, TARGET, "Constants", 1, false, "42");
            database.flush();
        }
    }

    private static void observe(Path path, String expected, int queryCount) throws Exception {
        byte[] before = Files.readAllBytes(path);
        String file = path.getFileName().toString();
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            require(database.getQueries().size() == queryCount, "wrong query count");
            for (Query query : database.getQueries()) {
                require(query.getType() == Query.Type.SELECT, "unexpected active query kind");
                System.out.printf("query\t%s\t%d\t%s\t%s%n", file, query.getObjectId(),
                        text(query.getName()), text(query.toSQLString()));
                if (query.getObjectId() == TARGET) {
                    require(query.toSQLString().contains(expected + " AS Answer"), "target literal mismatch");
                }
            }
            for (String name : new String[] { "MSysObjects", "MSysQueries", "LocalData" }) {
                Table table = name.startsWith("MSys") ? database.getSystemTable(name) : database.getTable(name);
                System.out.printf("table\t%s\t%s\t%d\t%d%n", file, name,
                        ((TableImpl) table).getTableDefPageNumber(), table.getRowCount());
                for (Column column : table.getColumns()) {
                    System.out.printf("column\t%s\t%s\t%s\t%s\t%d\t%s%n", file, name,
                            column.getName(), column.getType(), column.getColumnIndex(), column.isVariableLength());
                }
                for (Index index : table.getIndexes()) {
                    StringBuilder keys = new StringBuilder();
                    int rows = 0;
                    for (Row row : index.newCursor().toCursor()) {
                        RowIdImpl reference = (RowIdImpl) row.getId();
                        keys.append(reference.getPageNumber()).append(':').append(reference.getRowNumber());
                        for (Index.Column column : index.getColumns()) {
                            Object value = row.get(column.getName());
                            keys.append('|').append(value instanceof byte[]
                                    ? HexFormat.of().formatHex((byte[]) value) : text(value));
                        }
                        keys.append('\n');
                        rows++;
                    }
                    System.out.printf("index\t%s\t%s\t%s\t%d\t%d\t%s\t%s\t%s%n",
                            file, name, index.getName(), ((IndexImpl) index).getIndexNumber(),
                            Byte.toUnsignedInt(((IndexImpl) index).getIndexFlags()), index.isPrimaryKey(),
                            index.isUnique(), rows + ":" + hash(keys.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)));
                }
                for (Row row : table) {
                    RowIdImpl reference = (RowIdImpl) row.getId();
                    if (name.equals("MSysQueries")) {
                        Object rawOrder = row.get("Order");
                        String order = rawOrder == null ? "NULL" : HexFormat.of().formatHex((byte[]) rawOrder);
                        System.out.printf("fragment\t%s\t%d\t%d\t%d\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                                file, reference.getPageNumber(), reference.getRowNumber(), row.getInt("ObjectId"),
                                text(row.get("Attribute")), order, text(row.get("Expression")), text(row.get("Name1")),
                                text(row.get("Name2")), text(row.get("Flag")), text(row.get("LvExtra")));
                    } else if (name.equals("LocalData")) {
                        System.out.printf("local\t%s\t%d\t%d\t%d\t%d%n", file,
                                reference.getPageNumber(), reference.getRowNumber(), row.getInt("ID"), row.getInt("Score"));
                    } else if (Integer.valueOf(TARGET).equals(row.get("Id"))) {
                        StringBuilder nonnull = new StringBuilder();
                        for (String field : new String[] { "Owner", "Database", "Connect", "Lv", "LvProp", "LvModule", "LvExtra" }) {
                            if (row.get(field) != null) {
                                nonnull.append(field).append(',');
                            }
                        }
                        System.out.printf("catalog-target\t%s\t%d\t%d\t%d\t%d\t%s\t%s\t%s%n",
                                file, reference.getPageNumber(), reference.getRowNumber(), row.getInt("Id"),
                                row.getInt("ParentId"), text(row.get("DateCreate")), text(row.get("DateUpdate")), nonnull);
                    }
                }
            }
        }
        require(java.util.Arrays.equals(before, Files.readAllBytes(path)), "read-only observation changed input");
    }

    private static void updateField(Path original, Path after, int objectId, byte attribute,
                                    String expected, String replacement, String targetLiteral,
                                    int queryCount) throws Exception {
        require(!Files.exists(after), "fresh output path required");
        byte[] source = Files.readAllBytes(original);
        Files.copy(original, after);
        try (Database database = new DatabaseBuilder(after).open()) {
            database.setEvaluateExpressions(false);
            Table fragments = database.getSystemTable("MSysQueries");
            int changed = 0;
            for (Row row : fragments) {
                if (Integer.valueOf(objectId).equals(row.get("ObjectId"))
                        && Byte.valueOf(attribute).equals(row.get("Attribute"))) {
                    require(expected.equals(row.getString("Expression")), "unexpected source expression");
                    row.put("Expression", replacement);
                    fragments.updateRow(row);
                    changed++;
                }
            }
            require(changed == 1, "expected exactly one selected ObjectId/Column update");
            database.flush();
        }
        require(java.util.Arrays.equals(source, Files.readAllBytes(original)), "source modified by SDK update");
        observe(after, targetLiteral, queryCount);
    }

    private static void update(Path original, Path after, String replacement, int queryCount) throws Exception {
        updateField(original, after, TARGET, (byte) 6, "42", replacement, replacement, queryCount);
    }

    private record Edit(int objectId, byte attribute, int order, String expected,
                        String replacement) {}

    private static void updateMany(Path original, Path after, Edit[] edits,
                                   String constantLiteral) throws Exception {
        require(!Files.exists(after), "fresh output required");
        byte[] source = Files.readAllBytes(original);
        Files.copy(original, after);
        try (Database database = new DatabaseBuilder(after).open()) {
            database.setEvaluateExpressions(false);
            Table fragments = database.getSystemTable("MSysQueries");
            for (Edit edit : edits) {
                int changed = 0;
                for (Row row : fragments) {
                    if (Integer.valueOf(edit.objectId()).equals(row.get("ObjectId"))
                            && Byte.valueOf(edit.attribute()).equals(row.get("Attribute"))
                            && java.util.Arrays.equals(order(edit.order()), (byte[]) row.get("Order"))) {
                        require(edit.expected().equals(row.getString("Expression")), "source mismatch");
                        row.put("Expression", edit.replacement());
                        fragments.updateRow(row);
                        changed++;
                    }
                }
                require(changed == 1, "exact native fragment key must be unique");
            }
            database.flush();
        }
        require(java.util.Arrays.equals(source, Files.readAllBytes(original)), "original changed");
        observe(after, constantLiteral, 3);
    }

    public static void main(String[] args) throws Exception {
        require(args.length == 2, "expected retained original and fresh output directory");
        Path original = Path.of(args[0]);
        Path directory = Path.of(args[1]);
        Files.createDirectories(directory);
        observe(original, "42", 3);
        Path grow = directory.resolve("repeat-grow.accdb");
        Path shrink = directory.resolve("repeat-shrink.accdb");
        Path again = directory.resolve("repeat-grow-again.accdb");
        updateMany(original, grow, new Edit[] { new Edit(TARGET, (byte) 6, 1, "42", "1000") }, "1000");
        updateMany(grow, shrink, new Edit[] { new Edit(TARGET, (byte) 6, 1, "1000", "7") }, "7");
        updateMany(shrink, again, new Edit[] { new Edit(TARGET, (byte) 6, 1, "7", "1000000") }, "1000000");
        updateMany(again, directory.resolve("repeat-noop.accdb"),
                new Edit[] { new Edit(TARGET, (byte) 6, 1, "1000000", "1000000") }, "1000000");
        updateMany(original, directory.resolve("two-projections.accdb"), new Edit[] {
                new Edit(WIDE, (byte) 6, 96, "95", "1000"),
                new Edit(WIDE, (byte) 6, 95, "94", "7000") }, "42");
        updateMany(original, directory.resolve("projection-where.accdb"), new Edit[] {
                new Edit(WIDE, (byte) 6, 96, "95", "1000"),
                new Edit(WIDE, (byte) 8, 1, "[p].[ID] >= 2", "[p].[ID] >= 1000") }, "42");
        observe(original, "42", 3);
    }
}
