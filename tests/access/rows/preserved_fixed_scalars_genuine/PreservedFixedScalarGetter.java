import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Index;
import com.healthmarketscience.jackcess.PropertyMap;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.complex.Attachment;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.IndexData;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.TableImpl;
import com.healthmarketscience.jackcess.impl.UsageMap;
import com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.math.BigDecimal;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.time.LocalDateTime;
import java.util.Base64;
import java.util.HexFormat;

/** Bounded read-only inventory for the reviewed fixed-scalar rich profile. */
public final class PreservedFixedScalarGetter {
    private static final long FILE_BYTES = 2L * 1024 * 1024;
    private static final int MAX_COLUMNS = 32;
    private static final int MAX_PROPERTIES = 128;
    private static final int MAX_INDEXES = 16;
    private static final int CURRENCY_SCALE = 4;

    private PreservedFixedScalarGetter() {
    }

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new IOException("Linked targets are outside this original recipe");
        });
    }

    private static String digest(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static String cell(Object value) throws Exception {
        if (value == null) {
            return "N";
        }
        if (value instanceof Integer integer) {
            return "I32:" + integer;
        }
        if (value instanceof Byte number) {
            return "U8:" + Byte.toUnsignedInt(number);
        }
        if (value instanceof Short number) {
            return "I16:" + number;
        }
        if (value instanceof Long number) {
            return "I64:" + number;
        }
        if (value instanceof BigDecimal number) {
            if (number.scale() != CURRENCY_SCALE) {
                throw new IllegalArgumentException("Currency getter requires exact scale 4");
            }
            // Establish the native signed scaled-integer range without rounding.
            number.unscaledValue().longValueExact();
            return "CUR:" + number.toPlainString();
        }
        if (value instanceof Boolean flag) {
            return flag ? "B1" : "B0";
        }
        if (value instanceof String text) {
            return "T:" + Base64.getEncoder().encodeToString(text.getBytes(StandardCharsets.UTF_8));
        }
        if (value instanceof byte[] binary) {
            return "X:" + binary.length + ":" + digest(binary);
        }
        if (value instanceof LocalDateTime timestamp) {
            return "LDT:" + timestamp;
        }
        if (value instanceof ComplexValueForeignKey key) {
            return "FK:" + key.get();
        }
        throw new IllegalArgumentException("Unexpected source getter carrier");
    }

    private static void properties(String table, String owner, PropertyMap map) throws Exception {
        int count = 0;
        for (PropertyMap.Property property : map) {
            if (++count > MAX_PROPERTIES) {
                throw new IOException("Source property inventory exceeded its finite bound");
            }
            System.out.println("PROPERTY\t" + table + "\t" + owner + "\t"
                + property.getName() + "\t" + Byte.toUnsignedInt(property.getType().getValue())
                + "\t" + (property.isDdl() ? 1 : 0) + "\t" + cell(property.getValue()));
        }
    }

    // Version-pinned readonly implementation observations, independently
    // crosschecked against raw original bytes. No setter or page write is used.
    private static int mapFact(UsageMap map, String getter) throws Exception {
        Method method = UsageMap.class.getDeclaredMethod(getter);
        method.setAccessible(true);
        return ((Number) method.invoke(map)).intValue();
    }

    private static UsageMap ownedMap(IndexData data) throws Exception {
        Field field = IndexData.class.getDeclaredField("_ownedPages");
        field.setAccessible(true);
        return (UsageMap) field.get(data);
    }

    private static int indexRoot(IndexData data) throws Exception {
        Method method = IndexData.class.getDeclaredMethod("getRootPageNumber");
        method.setAccessible(true);
        return ((Number) method.invoke(data)).intValue();
    }

    private static void inspect(Path path) throws Exception {
        try (FileChannel raw = FileChannel.open(path, StandardOpenOption.READ);
             BoundedFileChannel bounded = new BoundedFileChannel(raw, FILE_BYTES);
             Database database = new DatabaseBuilder(path.toFile()).setChannel(bounded)
                .setReadOnly(true).open()) {
            inert(database);
            Table parent = database.getTable("PreservedRows");
            ComplexColumnInfoImpl<?> files =
                (ComplexColumnInfoImpl<?>) parent.getColumn("Files").getComplexInfo();
            Table flat = files.getComplexValueForeignKeyColumn().getTable();
            System.out.println("COMPLEX\tPreservedRows\tFiles\t"
                + ((TableImpl) flat).getTableDefPageNumber()
                + "\t" + ((ColumnImpl) files.getPrimaryKeyColumn()).getColumnNumber()
                + "\t" + ((ColumnImpl) files.getComplexValueForeignKeyColumn()).getColumnNumber());
            for (Table table : new Table[] {parent, database.getTable("Sentinel"), flat}) {
                String name = table.getName();
                int expectedRows = name.equals("Sentinel") ? 1 : 3;
                if (table.getRowCount() != expectedRows
                        || table.getColumnCount() > MAX_COLUMNS
                        || table.getIndexes().size() > MAX_INDEXES) {
                    throw new IOException("Source table inventory exceeded its finite profile");
                }
                TableImpl nativeTable = (TableImpl) table;
                UsageMap ordinary = nativeTable.getOwnedPagesCursor().getUsageMap();
                System.out.println("TABLE\t" + name + "\t" + table.getRowCount()
                    + "\t" + nativeTable.getTableDefPageNumber()
                    + "\t" + nativeTable.getMaxColumnCount());
                System.out.println("OWNED_MAP\t" + name
                    + "\t" + mapFact(ordinary, "getTablePageNumber")
                    + "\t" + mapFact(ordinary, "getRowStart"));
                properties(name, "", table.getProperties());
                for (Column column : table.getColumns()) {
                    ColumnImpl nativeColumn = (ColumnImpl) column;
                    System.out.println("COLUMN\t" + name + "\t" + column.getName() + "\t"
                        + Byte.toUnsignedInt(column.getType().getValue())
                        + "\t" + nativeColumn.getColumnNumber()
                        + "\t" + column.getColumnIndex()
                        + "\t" + nativeColumn.isVariableLength()
                        + "\t" + nativeColumn.getFixedDataOffset()
                        + "\t" + nativeColumn.getVarLenTableIndex()
                        + "\t" + nativeColumn.getLength()
                        + "\t" + column.isCalculated()
                        + "\t" + column.isAutoNumber());
                    properties(name, column.getName(), column.getProperties());
                }
                for (Index index : table.getIndexes()) {
                    StringBuilder keys = new StringBuilder();
                    for (Index.Column column : index.getColumns()) {
                        if (!keys.isEmpty()) {
                            keys.append(',');
                        }
                        keys.append(column.getName()).append(':').append(column.isAscending());
                    }
                    IndexData data = ((IndexImpl) index).getIndexData();
                    UsageMap owned = ownedMap(data);
                    System.out.println("INDEX\t" + name + "\t" + index.getName() + "\t"
                        + index.isPrimaryKey() + "\t" + index.isUnique() + "\t" + keys
                        + "\t" + data.getIndexDataNumber()
                        + "\t" + indexRoot(data)
                        + "\t" + mapFact(owned, "getTablePageNumber")
                        + "\t" + mapFact(owned, "getRowStart")
                        + "\t" + Byte.toUnsignedInt(data.getIndexFlags())
                        + "\t" + data.getUniqueEntryCount());
                }
                int ordinal = 0;
                for (Row row : table) {
                    if (ordinal >= expectedRows) {
                        throw new IOException("Source row inventory exceeded its declared recipe");
                    }
                    RowIdImpl rowId = (RowIdImpl) row.getId();
                    StringBuilder values = new StringBuilder("ROW\t").append(name).append('\t').append(ordinal)
                        .append('\t').append(rowId.getPageNumber()).append(':').append(rowId.getRowNumber());
                    for (Column column : table.getColumns()) {
                        values.append('\t').append(cell(column.getRowValue(row)));
                    }
                    System.out.println(values);
                    if (name.equals("PreservedRows")) {
                        ComplexValueForeignKey key = (ComplexValueForeignKey) row.get("Files");
                        for (Attachment attachment : key.getAttachments()) {
                            byte[] encoded = attachment.getEncodedFileData();
                            byte[] decoded = attachment.getFileData();
                            System.out.println("ATTACHMENT\t" + ordinal + "\t" + attachment.getId().get()
                                + "\t" + cell(attachment.getFileUrl()) + "\t" + cell(attachment.getFileName())
                                + "\t" + cell(attachment.getFileType()) + "\t" + cell(encoded) + "\t" + cell(decoded)
                                + "\t" + attachment.getFileTimeStampObject() + "\t" + attachment.getFileFlags());
                        }
                    }
                    ordinal++;
                }
                if (ordinal != expectedRows) {
                    throw new IOException("Source row inventory did not complete its recipe");
                }
            }
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2 || !arguments[0].equals("--inspect")) {
            throw new IllegalArgumentException("Expected --inspect existing-path");
        }
        inspect(Path.of(arguments[1]));
    }
}
