import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.Attachment;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.time.LocalDateTime;
import java.util.Arrays;
import java.util.HexFormat;

/** Original inert file-size corpus; generation refuses existing outputs. */
public class LargeDecodedAttachmentFixture {
    private static final int COMPRESSED_FILE_BYTES = 3_145_728;
    private static final int STORED_FILE_BYTES = 1_500_000;

    private static void inspect(Path path) throws Exception {
        try (Database db = new DatabaseBuilder(path).setReadOnly(true).open()) {
            db.setEvaluateExpressions(false);
            for (Row row : db.getTable("LargeFiles")) {
                var key = (ComplexValueForeignKey) row.get("Files");
                for (Attachment attachment : key.getAttachments()) {
                    byte[] data = attachment.getFileData();
                    byte[] encoded = attachment.getEncodedFileData();
                    String digest = HexFormat.of().formatHex(
                        MessageDigest.getInstance("SHA-256").digest(data));
                    int encoding = ByteBuffer.wrap(encoded).order(ByteOrder.LITTLE_ENDIAN).getInt();
                    System.out.printf("%d\t%s\t%d\t%s\t%d\t%d%n",
                        row.getInt("ID"), attachment.getFileName(), data.length,
                        digest, encoding, encoded.length);
                }
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and format, or --inspect path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }
        if (!Arrays.asList("V2007", "V2016", "V2019").contains(args[1])) {
            throw new IllegalArgumentException("Unsupported attachment profile");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        var format = Database.FileFormat.valueOf(args[1]);
        try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
            db.setEvaluateExpressions(false);
            Table table = new TableBuilder("LargeFiles")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Files").setAttachment())
                .toTable(db);
            byte[] compressed = new byte[COMPRESSED_FILE_BYTES];
            Arrays.fill(compressed, (byte) 42);
            byte[] stored = new byte[STORED_FILE_BYTES];
            for (int i = 0; i < stored.length; i++) {
                stored[i] = (byte) (i * 17 + i / 256);
            }
            LocalDateTime time = LocalDateTime.of(2026, 10, 24, 12, 0);
            Object[] first = table.addRow(1, Column.AUTO_NUMBER);
            var firstKey = (ComplexValueForeignKey) table.getColumn("Files").getRowValue(first);
            firstKey.addAttachment("", "large.txt", "txt", compressed, time, 0);
            Object[] second = table.addRow(2, Column.AUTO_NUMBER);
            var secondKey = (ComplexValueForeignKey) table.getColumn("Files").getRowValue(second);
            secondKey.addAttachment("", "stored.zip", "zip", stored, time, 0);
            table.addRow(3, Column.AUTO_NUMBER);
        }
    }
}
