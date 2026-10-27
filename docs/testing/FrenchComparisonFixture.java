package com.healthmarketscience.jackcess.impl;

import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.HexFormat;
import java.util.List;
import java.util.TimeZone;

/** Original inert field-data corpus; no SQL or string comparator is invoked. */
public final class FrenchComparisonFixture {
    private static final String TABLE = "Comparisons";
    private static final short FRENCH_LCID = 1036;
    private static final short GENERAL_LCID = 1033;
    private static final short VARIANT = 0;
    private static final short LEGACY_VERSION = 0;
    private static final int MAX_IMAGE_BYTES = 8 * 1024 * 1024;
    private static final List<String> FIELDS = List.of(
        "FrenchShort", "FrenchMemo", "GeneralShort", "GeneralMemo"
    );
    private static final String[] BASE = {
        null, "", "cote", "coté", "côte", "côté", "COTE", "é", "e\u0301", "œ",
        "oe", "æ", "ae", "ß", "ss", "a-b", "a'b", "a b", "a.b", "😀"
    };

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setTimeZone(TimeZone.getTimeZone("UTC"));
        database.setLinkResolver((source, token) -> {
            throw new AssertionError("External database is not authorized");
        });
    }

    private static String hash(byte[] value) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(value));
    }

    private static byte[] units(String value) {
        ByteBuffer bytes = ByteBuffer.allocate(value.length() * Character.BYTES)
            .order(ByteOrder.LITTLE_ENDIAN);
        for (int index = 0; index < value.length(); index++) {
            bytes.putChar(value.charAt(index));
        }
        return bytes.array();
    }

    private static List<String[]> cases() {
        List<String[]> result = new ArrayList<>();
        for (String value : BASE) {
            result.add(new String[]{value, value});
        }
        for (int length : new int[]{256, 1024, 20000}) {
            result.add(new String[]{"long-primary", "e".repeat(length - 1) + "a"});
            result.add(new String[]{"long-primary", "e".repeat(length - 1) + "b"});
            result.add(new String[]{"long-accent", "é" + "e".repeat(length - 1)});
            result.add(new String[]{"long-accent", "e".repeat(length - 1) + "é"});
            result.add(new String[]{"long-accent", "è" + "e".repeat(length - 2) + "é"});
            result.add(new String[]{"long-accent", "é" + "e".repeat(length - 2) + "è"});
        }
        return result;
    }

    private static ColumnBuilder field(String name, DataType type, short lcid) {
        ColumnBuilder column = new ColumnBuilder(name, type);
        if (type == DataType.TEXT) {
            column.setLengthInUnits(255);
        }
        column.setTextSortOrder(new ColumnImpl.SortOrder(lcid, VARIANT, LEGACY_VERSION));
        return column;
    }

    private static void generate(Path path, String profile) throws Exception {
        if (Files.exists(path) || !List.of("V2016", "V2019").contains(profile)) {
            throw new IllegalArgumentException("Expected fresh ACE16 output");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(profile)).create()) {
            inert(database);
            Table table = new TableBuilder(TABLE)
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(field(FIELDS.get(0), DataType.TEXT, FRENCH_LCID))
                .addColumn(field(FIELDS.get(1), DataType.MEMO, FRENCH_LCID))
                .addColumn(field(FIELDS.get(2), DataType.TEXT, GENERAL_LCID))
                .addColumn(field(FIELDS.get(3), DataType.MEMO, GENERAL_LCID))
                .addIndex(new IndexBuilder("Primary").addColumns("ID").setPrimaryKey())
                .addIndex(new IndexBuilder("FrenchShortAsc").addColumns("FrenchShort"))
                .toTable(database);
            int id = 1;
            for (String[] item : cases()) {
                table.addRow(id, item[0], item[1], item[0], item[1]);
                id++;
            }
        }
    }

    private static void properties(String owner, PropertyMap map) {
        for (PropertyMap.Property property : map) {
            String value = Base64.getEncoder().encodeToString(units(String.valueOf(property.getValue())));
            System.out.printf("PROPERTY\t%s\t%s\t%s\t%s\t%s%n", owner,
                property.getName(), property.getType(), property.isDdl(), value);
        }
    }

    private static void oracle(Path path) throws Exception {
        if (Files.size(path) > MAX_IMAGE_BYTES) {
            throw new IllegalArgumentException("Image exceeds fixture size ceiling");
        }
        byte[] raw = Files.readAllBytes(path);
        String before = hash(raw);
        System.out.printf("FILE\t%s\t%d\t%s%n", path.getFileName(), raw.length, before);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            inert(database);
            Table table = database.getTable(TABLE);
            TableImpl physical = (TableImpl) table;
            JetFormat format = physical.getFormat();
            int base = Math.multiplyExact(physical.getTableDefPageNumber(), format.PAGE_SIZE);
            ByteBuffer bytes = ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN);
            int indexCount = bytes.getInt(base + format.OFFSET_NUM_INDEXES);
            int columns = base + format.OFFSET_INDEX_DEF_BLOCK
                + indexCount * format.SIZE_INDEX_DEFINITION;
            System.out.printf("TABLE\t%s\t%d\t%d%n", TABLE, table.getRowCount(), physical.getTableDefPageNumber());
            properties(TABLE, table.getProperties());
            for (Column value : table.getColumns()) {
                ColumnImpl column = (ColumnImpl) value;
                int declaration = columns + column.getColumnIndex() * format.SIZE_COLUMN_HEADER;
                System.out.printf("COLUMN\t%s\t%s\t%d\t%d\t%d\t%d\t%s%n", column.getName(),
                    column.getType(), Byte.toUnsignedInt(raw[declaration + format.OFFSET_COLUMN_TYPE]),
                    column.getLength(), Byte.toUnsignedInt(raw[declaration + format.OFFSET_COLUMN_FLAGS]),
                    Byte.toUnsignedInt(raw[declaration + format.OFFSET_COLUMN_EXT_FLAGS]), column.isAutoNumber());
                properties(column.getName(), column.getProperties());
                if (FIELDS.contains(column.getName())) {
                    ColumnImpl.SortOrder stored = column.getTextSortOrder();
                    int offset = declaration + format.OFFSET_COLUMN_SORT_ORDER;
                    byte[] descriptor = Arrays.copyOfRange(raw, offset, offset + format.SIZE_SORT_ORDER);
                    System.out.printf("SORT\t%s\t%d\t%d\t%d\t%d\t%s%n", column.getName(),
                        stored.getValue(), stored.getVariant(), stored.getVersion(), offset,
                        HexFormat.of().formatHex(descriptor));
                }
            }
            for (Row row : table) {
                RowIdImpl location = (RowIdImpl) row.getId();
                System.out.printf("ROW\t%d\t%d\t%d%n", row.getInt("ID"), location.getPageNumber(), location.getRowNumber());
                for (String field : FIELDS) {
                    String value = row.getString(field);
                    byte[] actual = value == null ? new byte[0] : units(value);
                    System.out.printf("VALUE\t%d\t%s\t%s\t%d\t%s\t%s%n", row.getInt("ID"), field,
                        value == null ? "NULL" : "TEXT", value == null ? 0 : value.length(),
                        hash(actual), Base64.getEncoder().encodeToString(actual));
                }
            }
            for (Index index : table.getIndexes()) {
                IndexData data = ((IndexImpl) index).getIndexData();
                System.out.printf("INDEX\t%s\t%d\t%d\t%d\t%d%n", index.getName(),
                    data.getIndexDataNumber(), data.getRootPageNumber(), data.getEntryCount(), data.getOwnedPageCount());
                Cursor cursor = CursorBuilder.createCursor(index);
                Row row;
                while ((row = cursor.getNextRow()) != null) {
                    System.out.printf("ORDER\t%s\t%d%n", index.getName(), row.getInt("ID"));
                }
            }
        }
        if (!before.equals(hash(Files.readAllBytes(path)))) {
            throw new AssertionError("Read-only getter changed original");
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 3 && args[0].equals("generate")) {
            generate(Path.of(args[1]), args[2]);
        } else if (args.length == 2 && args[0].equals("oracle")) {
            oracle(Path.of(args[1]));
        } else {
            throw new IllegalArgumentException("Expected generate PATH PROFILE or oracle PATH");
        }
    }
}
