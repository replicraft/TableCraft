import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.security.MessageDigest;
import java.time.LocalDateTime;
import java.util.Arrays;
import java.util.HexFormat;

/** Original inert larger-value corpus and independent read-only getter oracle. */
public class LargePayloadFixture {
    static String digest(byte[] bytes) throws Exception {
        return bytes == null ? "NULL" : HexFormat.of().formatHex(
            MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    static void inspect(Path path) throws Exception {
        try (Database db = new DatabaseBuilder(path).setReadOnly(true).open()) {
            db.setEvaluateExpressions(false);
            for (Row row : db.getTable("LargeValues")) {
                String memo = row.getString("Memo");
                byte[] text = memo == null ? null : memo.getBytes(StandardCharsets.UTF_8);
                byte[] binary = row.getBytes("Bytes");
                System.out.printf("V\t%d\t%d\t%s\t%d\t%s%n", row.getInt("ID"),
                    text == null ? -1 : text.length, digest(text),
                    binary == null ? -1 : binary.length, digest(binary));
            }
            if (db.getTable("LargeAttachments") != null) {
                for (Row row : db.getTable("LargeAttachments")) {
                    var key = (ComplexValueForeignKey) row.get("Files");
                    for (Attachment attachment : key.getAttachments()) {
                        byte[] data = attachment.getFileData();
                        int encoding = ByteBuffer.wrap(attachment.getEncodedFileData())
                            .order(ByteOrder.LITTLE_ENDIAN).getInt();
                        System.out.printf("A\t%s\t%d\t%s\t%d%n",
                            attachment.getFileName(), data.length, digest(data), encoding);
                    }
                }
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) throw new IllegalArgumentException("Expected fresh path and format, or --inspect path");
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
        var format = Database.FileFormat.valueOf(args[1]);
        if (!Arrays.asList("V2000", "V2007", "V2016", "V2019").contains(args[1])) {
            throw new IllegalArgumentException("Unsupported format");
        }
        try (Database db = new DatabaseBuilder(path).setFileFormat(format).create()) {
            db.setEvaluateExpressions(false);
            Table values = new TableBuilder("LargeValues")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Bytes", DataType.OLE)).toTable(db);
            byte[] binary = new byte[2_097_152];
            for (int i = 0; i < binary.length; i++) binary[i] = (byte) (i * 31 + i / 256);
            values.addRow(1, "a".repeat(1_048_576), binary);
            values.addRow(2, "é🦀".repeat(50_000), new byte[0]);
            values.addRow(3, null, null);
            if (format != Database.FileFormat.V2000) {
                Table attachments = new TableBuilder("LargeAttachments")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
                    .addColumn(new ColumnBuilder("Files").setAttachment()).toTable(db);
                Object[] parent = attachments.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
                var key = (ComplexValueForeignKey) attachments.getColumn("Files").getRowValue(parent);
                byte[] data = new byte[1_048_576];
                Arrays.fill(data, (byte) 42);
                var time = LocalDateTime.of(2024, 2, 29, 12, 0);
                key.addAttachment("", "compressed.txt", "txt", data, time, 0);
                for (int i = 0; i < data.length; i++) data[i] = (byte) (i * 17 + i / 256);
                key.addAttachment("", "stored.zip", "zip", data, time, 0);
                attachments.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
            }
        }
    }
}
