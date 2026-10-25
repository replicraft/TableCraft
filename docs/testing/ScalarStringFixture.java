import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.LocalDateTime;

/** Original ordinary-table inputs; no expressions, saved queries or active content. */
public final class ScalarStringFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh PATH FORMAT");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must not exist");
        }

        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            Table table = new TableBuilder("TextOperands")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Spelling", DataType.TEXT).setLengthInUnits(100))
                .addColumn(new ColumnBuilder("Number", DataType.LONG))
                .addColumn(new ColumnBuilder("Flag", DataType.BOOLEAN))
                .addColumn(new ColumnBuilder("Money", DataType.MONEY))
                .addColumn(new ColumnBuilder("D", DataType.SHORT_DATE_TIME)).toTable(database);
            String[] spellings = {
                "02", "10", "bad", "", "é", "-2", "2", "True", "1.25", "2000-01-02",
                "2 ", "0", "1,25", "01/02/2000", "02.01.2000", null,
            };
            for (int position = 0; position < spellings.length; position++) {
                table.addRow(position + 1, spellings[position], 2, true,
                    new BigDecimal("1.25"), LocalDateTime.of(2000, 1, 2, 0, 0));
            }
            table.addRow(spellings.length + 1, "2", null, false, null, null);
            database.flush();
        }
    }
}
