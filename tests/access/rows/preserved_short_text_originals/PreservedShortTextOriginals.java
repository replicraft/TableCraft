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
import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.LocalDateTime;

/** Creation-only synthetic originals; never opens or rewrites an existing image. */
public final class PreservedShortTextOriginals {
    private static final long FILE_BYTES = 2L * 1024 * 1024;
    private static final int TEXT_UNITS = 40;
    private static final int PAYLOAD_BYTES = 8192;
    private static final int PATTERN_MULTIPLIER = 37;
    private static final int PATTERN_DIVISOR = 251;
    private static final LocalDateTime ATTACHMENT_TIME =
        LocalDateTime.of(2024, 2, 29, 12, 0);

    private static void inert(Database database) {
        database.setEvaluateExpressions(false);
        database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
        database.setLinkResolver((source, token) -> {
            throw new IOException("Linked targets are outside this original recipe");
        });
    }

    private static byte[] payload() {
        byte[] bytes = new byte[PAYLOAD_BYTES];
        for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) (index * PATTERN_MULTIPLIER + index / PATTERN_DIVISOR);
        }
        return bytes;
    }

    private static void create(Path path, String profile) throws Exception {
        if (Files.exists(path, LinkOption.NOFOLLOW_LINKS)) {
            throw new IllegalArgumentException("Original output must be absent");
        }
        Database.FileFormat format = switch (profile) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported original profile");
        };
        try (FileChannel raw = FileChannel.open(path, StandardOpenOption.CREATE_NEW,
                StandardOpenOption.READ, StandardOpenOption.WRITE);
             BoundedFileChannel bounded = new BoundedFileChannel(raw, FILE_BYTES);
             Database database = new DatabaseBuilder(path.toFile()).setChannel(bounded)
                .setFileFormat(format).create()) {
            inert(database);
            Table table = new TableBuilder("PreservedRows")
                .putProperty("PublishToWeb", DataType.BYTE, (byte) 0)
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Short_Text", DataType.TEXT)
                    .setLengthInUnits(TEXT_UNITS)
                    .setCompressedUnicode(true)
                    .putProperty("Required", DataType.BOOLEAN, false)
                    .putProperty("AllowZeroLength", DataType.BOOLEAN, true))
                .addColumn(new ColumnBuilder("Base", DataType.LONG))
                .addColumn(new ColumnBuilder("Cached", DataType.LONG)
                    .setCalculatedInfo("[Base]*2"))
                .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO))
                .addColumn(new ColumnBuilder("Blob", DataType.OLE))
                .addColumn(new ColumnBuilder("Files").setAttachment())
                .addIndex(new IndexBuilder("IdPrimary").addColumns("ID").setPrimaryKey())
                .addIndex(new IndexBuilder("BaseAsc").addColumns("Base"))
                .toTable(database);

            // Explicit stale caches are supplied with expression evaluation off.
            Object[] first = table.addRow(
                1, "alpha", 7, 999, false, "preserved memo é🦀", payload(), Column.AUTO_NUMBER);
            Object[] second = table.addRow(
                2, null, 11, -333, true, null, new byte[0], Column.AUTO_NUMBER);
            table.addRow(3, "omega", 13, 0, false, "", null, Column.AUTO_NUMBER);
            ComplexValueForeignKey firstFiles =
                (ComplexValueForeignKey) table.getColumn("Files").getRowValue(first);
            firstFiles.addAttachment("", "empty.txt", "txt", new byte[0], ATTACHMENT_TIME, 0);
            firstFiles.addAttachment("", "payload.zip", "zip", payload(), ATTACHMENT_TIME, 0);
            ComplexValueForeignKey secondFiles =
                (ComplexValueForeignKey) table.getColumn("Files").getRowValue(second);
            secondFiles.addAttachment("", "unicode.txt", "txt",
                "é🦀".getBytes(StandardCharsets.UTF_8), ATTACHMENT_TIME, 0);

            Table sentinel = new TableBuilder("Sentinel")
                .putProperty("PublishToWeb", DataType.BYTE, (byte) 0)
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Marker", DataType.TEXT).setLengthInUnits(TEXT_UNITS))
                .addColumn(new ColumnBuilder("Bytes", DataType.OLE))
                .toTable(database);
            sentinel.addRow(23, "unchanged sentinel", new byte[] {(byte) 199, 0, (byte) 255});
        }
    }

    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        create(Path.of(arguments[0]), arguments[1]);
    }
}
