import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.complex.Attachment;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.TimeZone;

// Original read-only getter comparison. Metadata and decoded bytes stay separate.
public class LargeAttachmentOracle {
    private static String text(String value) {
        return value == null ? "N" : "T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || (!args[0].equals("--metadata") && !args[0].equals("--decoded"))) {
            throw new IllegalArgumentException("Expected --metadata/--decoded and file");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[1])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.DATE);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            Table table = database.getTable("ManyFiles");
            Column column = table.getColumn("Files");
            var info = (ComplexColumnInfoImpl<?>) column.getComplexInfo();
            var date = (ColumnImpl) info.getComplexValueForeignKeyColumn().getTable().getColumn("FileTimeStamp");
            int parent = 0;
            for (Row row : table) {
                var key = (ComplexValueForeignKey) row.get("Files");
                for (Attachment attachment : key.getAttachments()) {
                    if (args[0].equals("--decoded")) {
                        System.out.printf("%d\t%d\t%s\t%s%n", parent, attachment.getId().get(),
                            HexFormat.of().formatHex(attachment.getFileType().getBytes(StandardCharsets.UTF_8)),
                            HexFormat.of().formatHex(attachment.getFileData()));
                        continue;
                    }
                    Object timestamp = attachment.getFileTimeStampObject();
                    String bits = timestamp == null ? "N" : String.format("Y64:%016x",
                        date.write(timestamp, 0, ByteOrder.LITTLE_ENDIAN).getLong(0));
                    byte[] encoded = attachment.getEncodedFileData();
                    System.out.printf("%d\t%d\t%s\t%s\t%s\t%s\t%s\t%s%n",
                        parent, attachment.getId().get(), text(attachment.getFileUrl()),
                        text(attachment.getFileName()), text(attachment.getFileType()),
                        encoded == null ? "N" : "X" + HexFormat.of().formatHex(encoded), bits,
                        attachment.getFileFlags() == null ? "N" : "I32:" + attachment.getFileFlags());
                }
                parent++;
            }
        }
    }
}
