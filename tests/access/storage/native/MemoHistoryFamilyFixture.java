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

/** Original inert ordinary-Memo flag-family fixture; hyperlink/rich text remains raw. */
public class MemoHistoryFamilyFixture {
    private static String[] values(String family) {
        return switch (family) {
            case "plain" -> new String[] {"current ASCII", "current é🦀\0\r\n", "", null, "文🦀".repeat(4000)};
            case "hyperlink" -> new String[] {
                "Display#https://example.invalid/a#section#tip",
                "é🦀#https://example.invalid/b#part#literal tip", "", null,
                "Last#https://example.invalid/c##screen tip",
            };
            case "richtext" -> new String[] {
                "<div><b>current</b></div>", "<div>é🦀&amp;literal\r\n</div>", "", null,
                "<div>" + "文🦀".repeat(4000) + "</div>",
            };
            default -> throw new IllegalArgumentException("Unknown ordinary Memo family");
        };
    }

    private static void populate(
        Database database, String tableName, String memoName, boolean compressed, String family
    ) throws Exception {
        ColumnBuilder memo = new ColumnBuilder(memoName, DataType.MEMO)
            .setCompressedUnicode(compressed).setAppendOnly(true)
            .setHyperlink(family.equals("hyperlink"));
        if (family.equals("richtext")) {
            memo.putProperty("TextFormat", DataType.BYTE, (byte) 1);
        }
        Table table = new TableBuilder(tableName)
            .addColumn(new ColumnBuilder("ID", DataType.LONG)).addColumn(memo).toTable(database);
        Column current = table.getColumn(memoName);
        if (current.isCompressedUnicode() != compressed
            || current.isHyperlink() != family.equals("hyperlink")) {
            throw new AssertionError("Requested original ordinary-Memo flags were not persisted");
        }
        Column hidden = current.getVersionHistoryColumn();
        String[] currentValues = values(family);
        LocalDateTime time = LocalDateTime.of(2024, 2, 29, 12, 0);
        for (int index = 0; index < currentValues.length; index++) {
            Object[] added = table.addRow(index + 1, currentValues[index], Column.AUTO_NUMBER);
            ComplexValueForeignKey key = (ComplexValueForeignKey) hidden.getRowValue(added);
            if (index == 0) {
                key.addVersion("same", time);
                key.addVersion("Older#https://example.invalid/old#section#é🦀", time.minusDays(1));
                key.addVersion("same", time);
            } else if (index == 1) {
                key.addVersion("", time.plusDays(1));
                key.addVersion("<div>" + "文🦀".repeat(3000) + "</div>", time.minusDays(2));
            } else if (index == 3) {
                key.addVersion(null, (LocalDateTime) null);
            }
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("Expected fresh path, V2007/V2016/V2019 and plain/hyperlink/richtext");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh output required");
        }
        values(args[2]);
        Database.FileFormat format = switch (args[1]) {
            case "V2007" -> Database.FileFormat.V2007;
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            populate(database, "Histories", "Notes", false, args[2]);
            populate(database, "OtherHistories", "Details", true, args[2]);
        }
        System.out.println("Original ordinary-Memo/history family created with no expression or provider execution");
    }
}
