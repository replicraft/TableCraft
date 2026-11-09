package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.impl.query.QueryFormat;
import com.healthmarketscience.jackcess.impl.query.QueryImpl;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Base64;
import java.util.HashMap;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.Map;
import java.util.Set;
import java.util.TimeZone;
import java.util.TreeMap;

/** Original inert saved-SELECT companion recipe; no SQL is executed here. */
public final class FrenchSavedCompanion {
    private static final String TABLE = "Comparisons";
    private static final String QUERY = "FrenchFields";
    private static final short CONTAINER_KIND = 3;
    private static final short QUERY_KIND = 5;
    private static final int FIRST_QUERY_ID = Integer.MIN_VALUE + 4000;
    private static final int MAX_ID_ATTEMPTS = 128;
    private static final int MAX_IMAGE_BYTES = 8 * 1024 * 1024;
    private static final int DATA_PAGE_KIND = 1;
    private static final int DATA_PAGE_VERSION = 1;
    private static final int OWNER_OFFSET = 4;
    private static final byte[] LONG_MARKER = "LVAL".getBytes(StandardCharsets.US_ASCII);

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("No external database is authorized");
        });
    }

    private static byte[] image(Path path) throws Exception {
        if (Files.size(path) > MAX_IMAGE_BYTES) {
            throw new IllegalArgumentException("Image exceeds bounded fixture size");
        }
        return Files.readAllBytes(path);
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static String encoded(String value) {
        return value == null ? "NULL" : Base64.getEncoder()
            .encodeToString(value.getBytes(StandardCharsets.UTF_8));
    }

    private static byte[] order(int ordinal) {
        return ByteBuffer.allocate(Integer.BYTES).putInt(ordinal).array();
    }

    private static void fragment(Table queries, int object, byte attribute,
            String expression, Short flag, String name1, String name2, int ordinal)
            throws Exception {
        QueryImpl.Row row = new QueryImpl.Row(null, attribute, expression, flag,
            null, name1, name2, object, order(ordinal));
        queries.addRowFromMap(row.toTableRow());
    }

    private static void copy(Path source, Path target) throws Exception {
        String before = hash(image(source));
        Files.copy(source, target);
        try (Database database = new DatabaseBuilder(target).open()) {
            inert(database);
            database.setAllowAutoNumberInsert(true);
            Table catalog = database.getSystemTable("MSysObjects");
            Table queries = database.getSystemTable("MSysQueries");
            Integer parent = null;
            Set<Integer> ids = new HashSet<>();
            for (Row row : catalog) {
                ids.add(row.getInt("Id"));
                if ("Tables".equals(row.get("Name"))
                        && Short.valueOf(CONTAINER_KIND).equals(row.get("Type"))) {
                    if (parent != null) {
                        throw new AssertionError("Ambiguous reviewed namespace container");
                    }
                    parent = row.getInt("Id");
                }
                if (Short.valueOf(QUERY_KIND).equals(row.get("Type"))) {
                    throw new AssertionError("Expected original without saved queries");
                }
            }
            if (parent == null) {
                throw new AssertionError("Missing reviewed namespace container");
            }
            int object = FIRST_QUERY_ID;
            int attempt = 0;
            while (ids.contains(object) && attempt < MAX_ID_ATTEMPTS) {
                object++;
                attempt++;
            }
            if (ids.contains(object)) {
                throw new AssertionError("Fresh object-ID allowance exhausted");
            }
            Map<String, Object> row = new HashMap<>();
            row.put("Id", object);
            row.put("ParentId", parent);
            row.put("Name", QUERY);
            row.put("Type", QUERY_KIND);
            row.put("Flags", 0);
            catalog.addRowFromMap(row);

            fragment(queries, object, QueryFormat.START_ATTRIBUTE, null, null, null, null, 1);
            fragment(queries, object, QueryFormat.TYPE_ATTRIBUTE, null, Query.Type.SELECT.getValue(), null, null, 1);
            fragment(queries, object, QueryFormat.FLAG_ATTRIBUTE, null, (short) 0, null, null, 1);
            fragment(queries, object, QueryFormat.TABLE_ATTRIBUTE, null, null, TABLE, "src", 1);
            fragment(queries, object, QueryFormat.COLUMN_ATTRIBUTE, "[src].[ID]", null, null, null, 1);
            fragment(queries, object, QueryFormat.COLUMN_ATTRIBUTE, "[src].[FrenchShort]", null, null, null, 2);
            fragment(queries, object, QueryFormat.COLUMN_ATTRIBUTE, "[src].[FrenchMemo]", null, null, null, 3);
            fragment(queries, object, QueryFormat.END_ATTRIBUTE, null, null, null, null, 1);
            database.flush();
        }
        if (!before.equals(hash(image(source)))) {
            throw new AssertionError("Source changed during companion creation");
        }
    }

    private static void protect(Map<Integer, String> pages, int number, String role) {
        if (number < 0) {
            throw new AssertionError("Invalid protected page");
        }
        pages.merge(number, role, (left, right) -> left + "," + right);
    }

    private static void oracle(Path path) throws Exception {
        byte[] raw = image(path);
        String before = hash(raw);
        System.out.printf("FILE\t%s\t%d\t%s%n", path.getFileName(), raw.length, before);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            if (database.getQueries().size() > 1) {
                throw new AssertionError("Unexpected extra query definitions");
            }
            for (Query query : database.getQueries()) {
                if (!query.getName().equals(QUERY) || query.getType() != Query.Type.SELECT
                        || !query.getParameters().isEmpty() || query.isHidden()) {
                    throw new AssertionError("Unexpected saved definition");
                }
                System.out.printf("QUERY\t%s\t%s\t%d\t%d\t%s\t%s%n", query.getName(),
                    query.getType(), query.getObjectId(), query.getObjectFlag(),
                    encoded(query.getOwnerAccessType()), encoded(query.toSQLString()));
                for (PropertyMap.Property property : query.getProperties()) {
                    System.out.printf("QPROPERTY\t%s\t%s%n", property.getName(), encoded(String.valueOf(property.getValue())));
                }
                for (Row value : database.getSystemTable("MSysObjects")) {
                    if (Integer.valueOf(query.getObjectId()).equals(value.get("Id"))) {
                        System.out.printf("QOBJECT\t%d\t%d\t%s\t%s%n", value.getInt("Id"),
                            value.getInt("ParentId"), value.get("Type"), value.get("Flags"));
                    }
                }
                for (Row value : database.getSystemTable("MSysQueries")) {
                    QueryImpl.Row item = new QueryImpl.Row(value);
                    if (Integer.valueOf(query.getObjectId()).equals(item.objectId)) {
                        System.out.printf("FRAGMENT\t%s\t%s\t%s\t%s\t%s\t%s\t%s%n", item.attribute,
                            encoded(item.expression), item.flag, item.extra, encoded(item.name1),
                            encoded(item.name2), HexFormat.of().formatHex(item.order));
                    }
                }
            }

            TableImpl table = (TableImpl) database.getTable(TABLE);
            int width = table.getFormat().PAGE_SIZE;
            Map<Integer, String> protectedPages = new TreeMap<>();
            protect(protectedPages, table.getTableDefPageNumber(), "user-definition");
            UsageMap.PageCursor cursor = table.getOwnedPagesCursor();
            cursor.beforeFirst();
            int number;
            while ((number = cursor.getNextPage()) >= 0) {
                protect(protectedPages, number, "user-data");
            }
            for (Index index : table.getIndexes()) {
                IndexData data = ((IndexImpl) index).getIndexData();
                if (data.getOwnedPageCount() != 1) {
                    throw new AssertionError("Unexpected non-root-leaf original index");
                }
                protect(protectedPages, data.getRootPageNumber(), "user-index");
            }
            for (int page = 0; page < raw.length / width; page++) {
                int base = page * width;
                boolean longValue = raw[base] == DATA_PAGE_KIND
                    && raw[base + 1] == DATA_PAGE_VERSION;
                for (int index = 0; index < LONG_MARKER.length; index++) {
                    longValue &= raw[base + OWNER_OFFSET + index] == LONG_MARKER[index];
                }
                if (longValue) {
                    protect(protectedPages, page, "original-LVAL-signature");
                }
            }
            for (Map.Entry<Integer, String> item : protectedPages.entrySet()) {
                byte[] page = java.util.Arrays.copyOfRange(raw, item.getKey() * width,
                    (item.getKey() + 1) * width);
                System.out.printf("PROTECTED\t%d\t%s\t%s%n", item.getKey(), item.getValue(), hash(page));
            }
        }
        if (!before.equals(hash(image(path)))) {
            throw new AssertionError("Read-only getter changed input");
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("copy")) {
            copy(Path.of(args[1]), Path.of(args[2]));
        } else if (args.length == 2 && args[0].equals("oracle")) {
            oracle(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected copy SOURCE FRESH_OUTPUT or oracle PATH");
        }
    }
}
