import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.complex.ComplexValueForeignKey;
import java.math.BigDecimal;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.Date;
import java.util.TimeZone;

/** Original advanced-field schema/row evolution corpus; formulas never execute. */
@SuppressWarnings("deprecation")
public class AdvancedEvolutionFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported format");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.DATE);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            Table table = new TableBuilder("Evolution")
                .addColumn(new ColumnBuilder("ID", DataType.LONG))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(80))
                .addColumn(new ColumnBuilder("Ordinary", DataType.MEMO))
                .toTable(database);
            addRows(table, 1, 48);

            new ColumnBuilder("Choices").setMultiValue(DataType.LONG)
                .setValueList("0", "1").addToTable(table);
            new ColumnBuilder("Files").setAttachment().addToTable(table);
            new ColumnBuilder("Cached", DataType.LONG)
                .setCalculatedInfo("[ID]*2").addToTable(table);
            new ColumnBuilder("Notes", DataType.MEMO).setAppendOnly(true).addToTable(table);
            addRows(table, 49, 96);

            new ColumnBuilder("LaterMemo", DataType.MEMO)
                .setCalculatedInfo("[Label] & \"later\"").addToTable(table);
            new ColumnBuilder("Amount", DataType.NUMERIC)
                .setCalculatedInfo("[ID]/10000").addToTable(table);
            for (int bit = 0; bit < 8; bit++) {
                new ColumnBuilder("Flag" + bit, DataType.BOOLEAN).addToTable(table);
            }
            addRows(table, 97, 160);

            Column history = table.getColumn("Notes").getVersionHistoryColumn();
            for (Row row : table) {
                int identity = (Integer) row.get("ID");
                if (identity % 23 == 0 || identity % 13 == 1) {
                    continue;
                }
                if (identity % 3 == 0) {
                    row.put("Cached", 999000 + identity);
                }
                if (identity % 7 == 0) {
                    row.put("LaterMemo", "é🦀".repeat(identity % 2 == 0 ? 3000 : 3));
                    row.put("Amount", new BigDecimal("-123.450000"));
                }
                if (identity % 5 == 0) {
                    row.put("Ordinary", "grown:" + "ordinary".repeat(800));
                    row.put("Notes", "current-" + identity);
                }
                for (int bit = 0; bit < 8; bit++) {
                    if (identity % (bit + 2) == 0) {
                        row.put("Flag" + bit, true);
                    }
                }
                table.updateRow(row);

                ComplexValueForeignKey choices = (ComplexValueForeignKey) row.get("Choices");
                if (identity % 4 != 0) {
                    choices.addMultiValue(identity);
                    choices.addMultiValue(null);
                    choices.addMultiValue(identity);
                }
                if (identity % 17 == 0) {
                    choices.deleteAllValues();
                    choices.addMultiValue(-identity);
                }
                ComplexValueForeignKey files = (ComplexValueForeignKey) row.get("Files");
                if (identity % 6 != 0) {
                    byte[] bytes = new byte[identity % 11 == 0 ? 20000 : 256];
                    for (int offset = 0; offset < bytes.length; offset++) {
                        bytes[offset] = (byte) (identity + offset);
                    }
                    files.addAttachment("", "file-" + identity + ".bin", "bin", bytes,
                        Date.from(Instant.parse("2024-02-29T12:00:00Z")), identity);
                    if (identity % 10 == 0) {
                        files.addAttachment("", "empty-" + identity + ".txt", "txt", new byte[0], (Date) null, 0);
                    }
                }
                if (identity % 19 == 0) {
                    files.deleteAllValues();
                    files.addAttachment(
                        "", "replacement-" + identity + ".bin", "bin",
                        new byte[] {1, 2, 3}, (Date) null, 0);
                }
                if (identity % 9 == 0) {
                    ComplexValueForeignKey versions = (ComplexValueForeignKey) row.get(history.getName());
                    versions.addVersion("manual-" + identity, Date.from(Instant.parse("2024-01-01T00:00:00Z")));
                    versions.addVersion(null, Date.from(Instant.parse("2024-01-02T00:00:00Z")));
                }
            }
            var cursor = table.newCursor().toCursor();
            while (cursor.moveToNextRow()) {
                if (((Integer) cursor.getCurrentRow().get("ID")) % 23 == 0) {
                    cursor.deleteCurrentRow();
                }
            }
        }
    }

    private static void addRows(Table table, int first, int last) throws Exception {
        for (int identity = first; identity <= last; identity++) {
            var values = new java.util.LinkedHashMap<String, Object>();
            values.put("ID", identity);
            values.put("Label", "row-" + identity);
            values.put("Ordinary", identity % 4 == 0 ? "" : "before-" + identity);
            for (Column column : table.getColumns()) {
                if (column.getType() == DataType.COMPLEX_TYPE) {
                    values.put(column.getName(), Column.AUTO_NUMBER);
                }
            }
            table.addRowFromMap(values);
        }
    }
}
