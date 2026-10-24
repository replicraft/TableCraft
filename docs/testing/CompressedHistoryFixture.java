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
import java.time.LocalDateTime;

/** Original inert compressed-current-Memo/stored-history fixture; fresh files only. */
public class CompressedHistoryFixture {
    private static void populate(Database database, String name, String memo) throws Exception {
        Table table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder(memo, DataType.MEMO)
                .setCompressedUnicode(true).setAppendOnly(true))
            .toTable(database);
        Column current = table.getColumn(memo);
        Column hidden = current.getVersionHistoryColumn();
        if (!current.isCompressedUnicode()) {
            throw new AssertionError("Requested current-Memo compression was not persisted");
        }
        String[] values = {"current ASCII", "current é🦀\0\r\n", "", null, "文🦀".repeat(4000)};
        LocalDateTime time = LocalDateTime.of(2024, 2, 29, 12, 0);
        for (int index = 0; index < values.length; index++) {
            Object[] added = table.addRow(index + 1, values[index], Column.AUTO_NUMBER);
            ComplexValueForeignKey key = (ComplexValueForeignKey) hidden.getRowValue(added);
            if (index == 0) {
                key.addVersion("same", time);
                key.addVersion("older é🦀\0\r\n", time.minusDays(1));
                key.addVersion("same", time);
            } else if (index == 1) {
                key.addVersion("", time.plusDays(1));
                key.addVersion("文🦀".repeat(3000), time.minusDays(2));
            } else if (index == 3) {
                key.addVersion(null, (LocalDateTime) null);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2007/V2016/V2019");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            populate(database, "Histories", "Notes");
            populate(database, "OtherHistories", "Details");
        }
        System.out.println("Original compressed current Memos and stored histories created without evaluation");
    }
}
