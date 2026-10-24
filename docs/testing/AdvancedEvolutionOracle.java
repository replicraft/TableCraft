import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.complex.Attachment;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.RowIdImpl;
import com.healthmarketscience.jackcess.impl.complex.AttachmentColumnInfoImpl;
import com.healthmarketscience.jackcess.impl.complex.MultiValueColumnInfoImpl;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.math.BigDecimal;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.Objects;
import java.util.TimeZone;

/** Original independent stored-row oracle; only --orphan edits an explicit copy. */
public class AdvancedEvolutionOracle {
    private static final String[] FIELDS = {
        "ID", "Label", "Ordinary", "Cached", "Notes", "LaterMemo", "Amount",
        "Flag0", "Flag1", "Flag2", "Flag3", "Flag4", "Flag5", "Flag6", "Flag7"
    };

    public static void main(String[] args) throws Exception {
        boolean orphan = args.length == 2 && args[0].equals("--orphan");
        boolean invalidCount = !orphan && args.length != 1;
        boolean invalidCopy = orphan
            && !Path.of(args[1]).getFileName().toString().equals("orphan.accdb");
        if (invalidCount || invalidCopy) {
            throw new IllegalArgumentException("Expected original path, or --orphan disposable orphan.accdb copy");
        }
        Path path = Path.of(args[orphan ? 1 : 0]);
        try (Database database = new DatabaseBuilder(path).setReadOnly(!orphan).open()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.DATE);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            Table table = database.getTable("Evolution");
            MultiValueColumnInfoImpl choices = (MultiValueColumnInfoImpl) table.getColumn("Choices").getComplexInfo();
            if (orphan) {
                Table flat = choices.getComplexValueForeignKeyColumn().getTable();
                Row row = flat.iterator().next();
                row.put(choices.getComplexValueForeignKeyColumn().getName(), Integer.MAX_VALUE);
                flat.updateRow(row);
                System.out.println("Explicit disposable orphan created");
                return;
            }
            AttachmentColumnInfoImpl files = (AttachmentColumnInfoImpl) table.getColumn("Files").getComplexInfo();
            Column hidden = table.getColumn("Notes").getVersionHistoryColumn();
            VersionHistoryColumnInfoImpl history = (VersionHistoryColumnInfoImpl) hidden.getComplexInfo();
            System.out.printf("D\t%s\t%s\t%s%n",
                hidden.getName(), history.getValueColumn().getName(),
                history.getModifiedDateColumn().getName());
            int parent = 0;
            for (Row row : table) {
                StringBuilder output = new StringBuilder("R\t").append(parent);
                for (String field : FIELDS) {
                    output.append('\t').append(scalar(row.get(field), table.getColumn(field).getType()));
                }
                System.out.println(output);
                RowIdImpl location = (RowIdImpl) row.getId();
                System.out.printf("P\t%d\t%d\t%d%n", parent, location.getPageNumber(), location.getRowNumber());
                ComplexValueForeignKey key = (ComplexValueForeignKey) row.get("Choices");
                for (Row child : choices.getComplexValueForeignKeyColumn().getTable()) {
                    if (belongs(child, choices.getComplexValueForeignKeyColumn(), key)) {
                        System.out.printf("C\t%d\t%s\t%s%n",
                            parent, child.get(choices.getPrimaryKeyColumn().getName()),
                            scalar(child.get(choices.getValueColumn().getName()), DataType.LONG));
                    }
                }
                ComplexValueForeignKey fileKey = (ComplexValueForeignKey) row.get("Files");
                HashMap<Integer, Attachment> decoded = new HashMap<>();
                if (fileKey != null) {
                    for (Attachment attachment : fileKey.getAttachments()) {
                        decoded.put(attachment.getId().get(), attachment);
                    }
                }
                for (Row child : files.getComplexValueForeignKeyColumn().getTable()) {
                    if (belongs(child, files.getComplexValueForeignKeyColumn(), fileKey)) {
                        Integer identity = (Integer) child.get(files.getPrimaryKeyColumn().getName());
                        Attachment attachment = decoded.remove(identity);
                        if (attachment == null) {
                            throw new IllegalStateException("Flat and public attachment identities differ");
                        }
                        byte[] encoded = (byte[]) child.get(files.getFileDataColumn().getName());
                        if (!java.util.Arrays.equals(encoded, attachment.getEncodedFileData())) {
                            throw new IllegalStateException("Flat and public encoded bytes differ");
                        }
                        System.out.printf("A\t%d\t%d\t%s\t%s\t%s\t%s%n", parent, identity,
                            scalar(child.get(files.getFileNameColumn().getName()), DataType.TEXT),
                            scalar(child.get(files.getFileTypeColumn().getName()), DataType.TEXT),
                            binary(encoded), binary(attachment.getFileData()));
                    }
                }
                if (!decoded.isEmpty()) {
                    throw new IllegalStateException("Unmatched public attachments");
                }
                ComplexValueForeignKey historyKey = (ComplexValueForeignKey) row.get(hidden.getName());
                for (Row child : history.getComplexValueForeignKeyColumn().getTable()) {
                    if (belongs(child, history.getComplexValueForeignKeyColumn(), historyKey)) {
                        Object modified = child.get(history.getModifiedDateColumn().getName());
                        String date = "N";
                        if (modified != null) {
                            ColumnImpl column = (ColumnImpl) history.getModifiedDateColumn();
                            var stored = column.write(modified, 0, ByteOrder.LITTLE_ENDIAN).duplicate();
                            long bits = stored.order(ByteOrder.LITTLE_ENDIAN).getLong();
                            date = String.format("Y64:%016x", bits);
                        }
                        System.out.printf("H\t%d\t%s\t%s\t%s%n",
                            parent, child.get(history.getPrimaryKeyColumn().getName()),
                            scalar(child.get(history.getValueColumn().getName()), DataType.MEMO), date);
                    }
                }
                parent++;
            }
        }
    }

    private static boolean belongs(Row child, Column foreign, ComplexValueForeignKey key) {
        return key != null && Objects.equals(child.get(foreign.getName()), key.get());
    }

    private static String binary(byte[] bytes) {
        return bytes == null ? "N" : "X" + HexFormat.of().formatHex(bytes);
    }

    private static String scalar(Object value, DataType type) {
        if (value == null) {
            return "N";
        }
        return switch (type) {
            case LONG -> "I32:" + value;
            case BOOLEAN -> Boolean.TRUE.equals(value) ? "B1" : "B0";
            case TEXT, MEMO -> "T" + HexFormat.of().formatHex(((String) value).getBytes(StandardCharsets.UTF_8));
            case NUMERIC -> "Q" + ((BigDecimal) value).unscaledValue() + ":" + ((BigDecimal) value).scale();
            default -> throw new IllegalArgumentException("Unexpected original scalar carrier");
        };
    }
}
