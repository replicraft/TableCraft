import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.IndexBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.time.LocalDateTime;
import java.util.HexFormat;
import java.util.Random;

// Original inert metadata inputs. The large file is valid but exceeds TableCraft's
// current encoded-payload envelope; metadata must not load it to qualify a name.
public class AttachmentMetadataFixture {
    private static final int FILE_BYTES = 3 * 1024 * 1024;
    private static final long SEED = 20261023L;

    private static String text(String value) {
        return value == null ? "N" : "T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }

    private static void inspect(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            Table table = database.getTable("MetadataFiles");
            for (var row : table) {
                var key = (ComplexValueForeignKey) row.get("Files");
                var attachments = key.getAttachments();
                System.out.printf("P\t%d\t%d%n", row.get("ID"), attachments.size());
                for (var attachment : attachments) {
                    byte[] data = attachment.getFileData();
                    String timestamp = attachment.getFileLocalTimeStamp() == null
                        ? "N" : "D" + attachment.getFileLocalTimeStamp();
                    String flags = attachment.getFileFlags() == null ? "N" : "I" + attachment.getFileFlags();
                    String hash = HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
                    System.out.printf("A\t%d\t%s\t%s\t%s\t%s\t%s\t%d\t%d\t%s%n",
                        row.get("ID"), text(attachment.getFileUrl()), text(attachment.getFileName()),
                        text(attachment.getFileType()), timestamp, flags,
                        attachment.getEncodedFileData().length, data.length, hash);
                }
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh output/format or --inspect/input");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output already exists");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported fixture profile");
        };
        byte[] payload = new byte[FILE_BYTES];
        new Random(SEED).nextBytes(payload);
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            Table table = new TableBuilder("MetadataFiles")
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
                .addColumn(new ColumnBuilder("Files").setAttachment())
                .addIndex(new IndexBuilder("IdAsc").addColumns("ID").setPrimaryKey())
                .toTable(database);
            Object[] row = table.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
            var key = (ComplexValueForeignKey) table.getColumn("Files").getRowValue(row);
            var large = key.addAttachment("../inert?query", "large-é😀.bin", "bin", payload,
                LocalDateTime.of(2024, 1, 1, 12, 0), 7);
            if (large.getEncodedFileData().length <= 2 * 1024 * 1024) {
                throw new IllegalStateException("Fixture payload unexpectedly compressed below the test boundary");
            }
            key.addAttachment(null, "small.bin", null, new byte[] {1, 2, (byte) 255}, (LocalDateTime) null, null);
            table.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
        }
        System.out.println("Created original two-parent metadata input with a valid 3 MiB file.");
    }
}
