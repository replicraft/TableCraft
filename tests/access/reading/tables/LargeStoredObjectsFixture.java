import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.Date;
import java.util.TimeZone;

// Original inert attachment/history scale inputs; no execution or save qualification.
@SuppressWarnings("deprecation")
public class LargeStoredObjectsFixture {
    private static final int PARENTS = 200;
    private static final int CHILDREN = 100;
    private static final Instant BASE_TIME = Instant.parse("2024-01-01T12:00:00Z");
    private static final long SECONDS_PER_DAY = 86_400;

    public static void main(String[] args) throws Exception {
        if (args.length != 2 || Files.exists(Path.of(args[0]))) {
            throw new IllegalArgumentException("Expected fresh output path and format");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2007" -> Database.FileFormat.V2007;
            default -> throw new IllegalArgumentException("Unsupported engineering format");
        };
        try (Database database = new DatabaseBuilder(Path.of(args[0]))
                .setFileFormat(format).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.DATE);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            Table files = new TableBuilder("ManyFiles")
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
                .addColumn(new ColumnBuilder("Files").setAttachment())
                .toTable(database);
            Table histories = new TableBuilder("Histories")
                .addColumn(new ColumnBuilder("ID", DataType.LONG).setAutoNumber(true))
                .addColumn(new ColumnBuilder("Notes", DataType.MEMO).setAppendOnly(true))
                .toTable(database);
            Column hiddenHistory = histories.getColumn("Notes").getVersionHistoryColumn();

            for (int parent = 1; parent <= PARENTS + 1; parent++) {
                Object[] fileRow = files.addRow(Column.AUTO_NUMBER, Column.AUTO_NUMBER);
                var attachments = (ComplexValueForeignKey) files.getColumn("Files").getRowValue(fileRow);
                Object[] historyRow = histories.addRow(Column.AUTO_NUMBER, null);
                var versions = (ComplexValueForeignKey) hiddenHistory.getRowValue(historyRow);
                if (parent > PARENTS) {
                    continue;
                }
                for (int child = 0; child < CHILDREN; child++) {
                    Date time = Date.from(BASE_TIME.plusSeconds(child % 7 * SECONDS_PER_DAY));
                    attachments.addAttachment("", "f" + parent + "-" + child + ".bin", "bin",
                        new byte[] {(byte) parent, (byte) child, (byte) 255}, time, child % 3);
                    String text = child % 13 == 0 ? null : "v:" + parent + ":" + child % 7 + ":é😀";
                    versions.addVersion(text, child % 11 == 0 ? (Date) null : time);
                }
            }
        }
        System.out.println("Created 201 attachment parents / 20,000 attachments and 201 history parents / 20,000 versions.");
    }
}
