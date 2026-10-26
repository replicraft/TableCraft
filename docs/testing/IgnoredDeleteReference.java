package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.math.BigDecimal;
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

/** Original public-copy/delete recipe; no database generation or imported execution. */
public final class IgnoredDeleteReference {
    private static final int PAGE_SIZE = 4096;
    private static final int INDEX_HEIGHT_OFFSET = 26;
    private static final int MAX_REQUESTS = 701;

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
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
            for (Row row : selected) {
                table.deleteRow(row);
            }
        }
        System.out.printf("REFERENCE\t%s\t%s\t%d%n", target.getFileName(), tableName, ids.size());
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length == 3 && arguments[0].equals("oracle")) {
            oracle(Path.of(arguments[1]), List.of(arguments[2].split(",")));
        } else if (arguments.length == 5 && arguments[0].equals("reference")) {
            reference(Path.of(arguments[1]), Path.of(arguments[2]), arguments[3], Path.of(arguments[4]));
        } else {
            throw new IllegalArgumentException("Expected oracle PATH TABLES or reference SOURCE TARGET TABLE IDS");
        }
    }
}
