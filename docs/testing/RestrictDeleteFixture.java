package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Relationship;
import com.healthmarketscience.jackcess.RelationshipBuilder;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.ConstraintViolationException;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.List;
import java.util.Set;
import java.util.TimeZone;

/** Original inert RESTRICT recipe, public-copy actions and read-only getter observations. */
public final class RestrictDeleteFixture {
    private static final int PAGE_SIZE = 4096;
    private static final int INDEX_HEIGHT_OFFSET = 26;
    private static final int MAX_REQUESTS = 32;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setEnforceForeignKeys(true);
        if (!database.isEnforceForeignKeys()) {
            throw new AssertionError("Foreign keys must be enabled");
        }
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No linked targets authorized");
        });
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static String text(String value) {
        StringBuilder result = new StringBuilder();
        for (char unit : value.toCharArray()) {
            result.append(String.format("%04x", (int) unit));
        }
        return result.toString();
    }

    private static String value(Object value, DataType type) throws Exception {
        if (value == null) {
            return "NULL";
        }
        if (value instanceof byte[] bytes) {
            return type == DataType.OLE
                ? "OLE:" + bytes.length + ":" + hash(bytes)
                : "BIN:" + HexFormat.of().formatHex(bytes);
        }
        if (value instanceof BigDecimal decimal) {
            return "DEC:" + decimal.unscaledValue() + ":" + decimal.scale();
        }
        if (value instanceof String string) {
            return type == DataType.MEMO
                ? "MEMO:" + string.length() + ":" + hash(string.getBytes(StandardCharsets.UTF_8))
                : "TEXT:" + text(string);
        }
        return value.getClass().getSimpleName() + ":" + value;
    }

    private static void properties(String table, String owner, PropertyMap properties) throws Exception {
        for (PropertyMap.Property property : properties) {
            System.out.printf("PROPERTY\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n",
                table, owner, property.getName(), property.getType(), property.isDdl(),
                property.isSkipHandler(), value(property.getValue(), property.getType()));
        }
    }

    private static void catalog(Database database, String name) throws Exception {
        Table objects = database.getSystemTable("MSysObjects");
        Object identity = null;
        for (Row row : objects) {
            if (name.equals(row.get("Name"))) {
                identity = row.get("Id");
                System.out.printf("CATALOG\t%s\t%s\t%s\t%s%n",
                    name, identity, row.get("Type"), row.get("Flags"));
                for (String field : List.of("Lv", "LvModule", "LvExtra")) {
                    System.out.printf("ACTIVE\t%s\t%s\t%s\t%s%n", name, field,
                        row.containsKey(field), row.get(field) == null ? "NULL" : "PRESENT");
                }
            }
        }
        if (identity == null) {
            throw new AssertionError("Missing selected table catalog record");
        }
        for (Row row : objects) {
            if (identity.equals(row.get("ParentId"))) {
                System.out.printf("CHILD\t%s\t%s\t%s%n", name,
                    text(String.valueOf(row.get("Name"))), row.get("Type"));
            }
        }
    }

    private static void oracle(Path path, List<String> selected) throws Exception {
        byte[] raw = Files.readAllBytes(path);
        System.out.printf("FILE\t%s\t%d\t%s%n", path.getFileName(), raw.length, hash(raw));
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            System.out.printf("ENFORCEMENT\t%s%n", database.isEnforceForeignKeys());
            for (Relationship relationship : database.getRelationships()) {
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
            for (String name : selected) {
                Table table = database.getTable(name);
                TableImpl physical = (TableImpl) table;
                System.out.printf("TABLE\t%s\t%d\t%d\t%d\t%d%n", name, table.getRowCount(),
                    physical.getTableDefPageNumber(), physical.getColumnCount(), physical.getMaxColumnCount());
                catalog(database, name);
                System.out.printf("RELATIONSHIPS\t%s\t%d%n", name, database.getRelationships(table).size());
                properties(name, "TABLE", table.getProperties());
                for (Column column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    System.out.printf("COLUMN\t%s\t%s\t%d\t%s\t%d\t%s%n", name,
                        column.getName(), column.getColumnIndex(), column.getType(),
                        column.getLength(), nativeColumn.getTextSortOrder());
                    System.out.printf("NATIVE_TYPE\t%s\t%s\t%d\t%d\t%d%n", name,
                        column.getName(), Byte.toUnsignedInt(column.getType().getValue()),
                        column.getType() == DataType.NUMERIC ? column.getPrecision() : 0,
                        column.getType() == DataType.NUMERIC ? column.getScale() : 0);
                    properties(name, column.getName(), column.getProperties());
                }
                for (Row row : table) {
                    RowIdImpl location = (RowIdImpl) row.getId();
                    System.out.printf("ROW\t%s\t%d\t%d\t%d", name, row.getInt("ID"),
                        location.getPageNumber(), location.getRowNumber());
                    for (Column column : table.getColumns()) {
                        System.out.print("\t" + value(row.get(column.getName()), column.getType()));
                    }
                    System.out.println();
                }
                for (Index index : table.getIndexes()) {
                    IndexImpl nativeIndex = (IndexImpl) index;
                    IndexData data = nativeIndex.getIndexData();
                    int root = data.getRootPageNumber();
                    System.out.printf("INDEX\t%s\t%s\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%d\t%s\t%s\t%s%n",
                        name, index.getName(), data.getIndexDataNumber(), root,
                        Byte.toUnsignedInt(raw[root * PAGE_SIZE]),
                        Byte.toUnsignedInt(raw[root * PAGE_SIZE + INDEX_HEIGHT_OFFSET]),
                        data.getOwnedPageCount(), nativeIndex.getUniqueEntryCount(),
                        nativeIndex.getUniqueEntryCountOffset(), data.getEntryCount(),
                        index.isUnique(), index.shouldIgnoreNulls(), index.isRequired());
                    for (Index.Column column : index.getColumns()) {
                        System.out.printf("KEYCOLUMN\t%s\t%s\t%s\t%s%n", name,
                            index.getName(), column.getName(), column.isAscending());
                    }
                    Cursor cursor = CursorBuilder.createCursor(index);
                    Row row;
                    while ((row = cursor.getNextRow()) != null) {
                        System.out.printf("ORDER\t%s\t%s\t%d%n", name, index.getName(), row.getInt("ID"));
                    }
                    IndexData.EntryCursor entries = data.cursor();
                    for (int position = 0; position < data.getEntryCount(); position++) {
                        IndexData.Entry entry = entries.getNextEntry();
                        if (!entry.isValid() || !entry.isLeafEntry()) {
                            throw new AssertionError("Expected complete leaf cursor");
                        }
                        RowIdImpl location = entry.getRowId();
                        System.out.printf("KEY\t%s\t%s\t%d\t%d\t%s%n", name,
                            index.getName(), location.getPageNumber(), location.getRowNumber(),
                            HexFormat.of().formatHex(entry.getEntryBytes()));
                    }
                }
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked database handle");
            }
        }
    }

    private static void reference(Path source, Path target, String tableName, Path request) throws Exception {
        if (Files.exists(target) || source.toAbsolutePath().normalize().equals(target.toAbsolutePath().normalize())) {
            throw new IllegalArgumentException("Reference destination must be fresh");
        }
        Set<Integer> ids = new HashSet<>();
        for (String line : Files.readAllLines(request, StandardCharsets.US_ASCII)) {
            if (line.isEmpty() || !ids.add(Integer.parseInt(line))) {
                throw new IllegalArgumentException("Invalid or duplicate requested ID");
            }
        }
        if (ids.isEmpty() || ids.size() > MAX_REQUESTS) {
            throw new IllegalArgumentException("Reference request count is not admitted");
        }
        Files.copy(source, target);
        byte[] originalCopy = Files.readAllBytes(target);
        boolean rejected = false;
        try (Database database = new DatabaseBuilder(target).open()) {
            inert(database);
            Table table = database.getTable(tableName);
            List<Row> selected = new ArrayList<>();
            for (Row row : table) {
                if (ids.contains(row.getInt("ID"))) {
                    selected.add(row);
                }
            }
            if (selected.size() != ids.size()) {
                throw new AssertionError("Requested IDs are not unique complete live roots");
            }
            try {
                for (Row row : selected) {
                    table.deleteRow(row);
                }
            } catch (ConstraintViolationException expected) {
                rejected = true;
            }
        }
        if (rejected) {
            if (!java.util.Arrays.equals(originalCopy, Files.readAllBytes(target))) {
                throw new AssertionError("Rejected individual action changed the copy");
            }
            System.out.printf("REJECTED_CONSTRAINT_UNCHANGED\t%s\t%s%n", target.getFileName(), tableName);
            return;
        }
        System.out.printf("REFERENCE\t%s\t%s\t%d%n", target.getFileName(), tableName, ids.size());
    }

    private enum Kind {
        BYTE(DataType.BYTE), I16(DataType.INT), I32(DataType.LONG), I64(DataType.BIG_INT),
        CURRENCY(DataType.MONEY), GUID(DataType.GUID), DECIMAL(DataType.NUMERIC);

        final DataType type;

        Kind(DataType type) {
            this.type = type;
        }

        ColumnBuilder column(String name) {
            ColumnBuilder column = new ColumnBuilder(name, type);
            if (type == DataType.NUMERIC) {
                column.setPrecision(28).setScale(4);
            }
            return column;
        }

        Object value(int ordinal) {
            if (ordinal == 6) {
                return null;
            }
            if (type == DataType.GUID) {
                String[] values = {"", "00000000-0000-0000-0000-000000000000",
                    "00000000-0000-0000-0000-00000000000A", "00000000-0000-0000-0000-000000000014",
                    "00000000-0000-0000-0000-00000000001E", "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF",
                    "", "00000000-0000-0000-0000-000000000040", "00000000-0000-0000-0000-000000000041"};
                return "{" + values[ordinal] + "}";
            }
            long minimum = switch (type) {
                case BYTE -> 0;
                case INT -> Short.MIN_VALUE;
                case LONG -> Integer.MIN_VALUE;
                default -> Long.MIN_VALUE;
            };
            long maximum = switch (type) {
                case BYTE -> 255;
                case INT -> Short.MAX_VALUE;
                case LONG -> Integer.MAX_VALUE;
                default -> Long.MAX_VALUE;
            };
            long adjacent = type == DataType.BIG_INT || type == DataType.MONEY
                || type == DataType.NUMERIC ? 9007199254740992L : 31L;
            long scalar = switch (ordinal) {
                case 1 -> minimum;
                case 2 -> 10;
                case 3 -> 20;
                case 4 -> 30;
                case 5 -> maximum;
                case 7 -> adjacent;
                case 8 -> adjacent + 1;
                default -> throw new IllegalArgumentException("Unknown original key ordinal");
            };
            return switch (type) {
                case BYTE -> (int) scalar;
                case INT -> (short) scalar;
                case LONG -> (int) scalar;
                case BIG_INT -> scalar;
                case MONEY -> BigDecimal.valueOf(scalar, 4);
                case NUMERIC -> new BigDecimal(ordinal == 1
                    ? new BigInteger("-9999999999999999999999999999")
                    : ordinal == 5 ? new BigInteger("9999999999999999999999999999")
                    : BigInteger.valueOf(scalar), 4);
                default -> throw new IllegalArgumentException("Unknown original scalar kind");
            };
        }
    }

    private static Table table(Database database, String name, Kind[] kinds, boolean parent) throws Exception {
        TableBuilder builder = new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG));
        String[] keys = new String[kinds.length];
        for (int index = 0; index < kinds.length; index++) {
            keys[index] = kinds.length == 1 ? "KeyValue" : "K" + index;
            builder.addColumn(kinds[index].column(keys[index]));
        }
        builder.addIndex(new IndexBuilder("PrimaryID").addColumns("ID").setPrimaryKey());
        if (parent) {
            builder.addIndex(new IndexBuilder("UniqueNullableKey").addColumns(keys).setUnique());
        } else {
            builder.addIndex(new IndexBuilder("KeyAsc").addColumns(keys));
            builder.addIndex(new IndexBuilder("KeyAlias").addColumns(keys));
            IndexBuilder descending = new IndexBuilder("KeyDesc");
            for (String key : keys) {
                descending.addColumns(false, key);
            }
            builder.addIndex(descending);
        }
        return builder.toTable(database);
    }

    private static void relationship(Database database, Table parent, Table child, boolean updates) throws Exception {
        RelationshipBuilder builder = new RelationshipBuilder(parent, child)
            .setReferentialIntegrity().setName(parent.getName() + "_" + child.getName());
        for (Column column : parent.getColumns()) {
            if (!column.getName().equals("ID")) {
                builder.addColumns(column.getName(), column.getName());
            }
        }
        if (updates) {
            builder.setCascadeUpdates();
        }
        builder.toRelationship(database);
    }

    private static void generate(Path target, String profile) throws Exception {
        if (Files.exists(target)) {
            throw new IllegalArgumentException("Original destination must be new");
        }
        if (!List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Only explicit ACE16 profiles qualify");
        }
        try (Database database = new DatabaseBuilder(target)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            for (Kind kind : Kind.values()) {
                Table parent = table(database, kind + "Parents", new Kind[]{kind}, true);
                Table child = table(database, kind + "Children", new Kind[]{kind}, false);
                relationship(database, parent, child, kind.ordinal() % 2 != 0);
                for (int id = 1; id <= 8; id++) {
                    parent.addRow(id, kind.value(id));
                }
                int[] keys = {2, 2, 3, 6, 1, 5, 7};
                for (int index = 0; index < keys.length; index++) {
                    child.addRow(index + 1, kind.value(keys[index]));
                }
                if (kind == Kind.I32) {
                    Table second = table(database, "I32SecondChildren", new Kind[]{kind}, false);
                    relationship(database, parent, second, true);
                    second.addRow(1, kind.value(2));
                    second.addRow(2, (Object) null);
                }
            }
            for (boolean decimal : new boolean[]{false, true}) {
                String prefix = decimal ? "DecimalTuple" : "IntegerTuple";
                Kind[] kinds = decimal ? new Kind[]{Kind.DECIMAL, Kind.I32} : new Kind[]{Kind.I32, Kind.I64};
                Table parent = table(database, prefix + "Parents", kinds, true);
                Table child = table(database, prefix + "Children", kinds, false);
                relationship(database, parent, child, decimal);
                Object ten = decimal ? BigDecimal.valueOf(10, 4) : Integer.valueOf(10);
                Object twenty = decimal ? BigDecimal.valueOf(20, 4) : Integer.valueOf(20);
                Object hundred = decimal ? Integer.valueOf(100) : Long.valueOf(100);
                Object twoHundred = decimal ? Integer.valueOf(200) : Long.valueOf(200);
                Object[][] parentRows = {{1, ten, hundred}, {2, twenty, twoHundred},
                    {3, twenty, hundred}, {4, null, twoHundred}, {5, twenty, null},
                    {6, null, null}, {7, null, null}};
                Object[][] childRows = {{1, ten, hundred}, {2, twenty, twoHundred},
                    {3, null, null}, {4, null, twoHundred}, {5, twenty, null}, {6, ten, hundred}};
                for (Object[] row : parentRows) {
                    parent.addRow(row);
                }
                for (Object[] row : childRows) {
                    child.addRow(row);
                }
            }
        }
        System.out.printf("NEW_ORIGINAL\t%s\t%d\t%s%n", target.getFileName(), Files.size(target), hash(Files.readAllBytes(target)));
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("generate")) {
            generate(Path.of(arguments[1]), arguments[2]);
        } else if (arguments.length == 3 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]), List.of(arguments[2].split(",")));
        } else if (arguments.length == 5 && arguments[0].equals("reference")) {
            reference(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3], Path.of(arguments[4]));
        } else {
            throw new IllegalArgumentException("Expected generate/oracle/reference with explicit paths");
        }
    }
}
