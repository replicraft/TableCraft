import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original inert finite Single/Double fields, authored only through public APIs. */
public class NumericKeyFixture {
    private static void row(Table table, int family, int ordinal, Float single, Double real) throws Exception {
        table.addRow(family * 10 + ordinal, family, ordinal, single != null, single, real);
    }

    public static void main(String[] arguments) throws Exception {
        Database.FileFormat format = Database.FileFormat.valueOf(arguments[1]);
        Path path = Path.of(arguments[0]);
        if (Files.exists(path)) {
            throw new IllegalStateException("Refuse to overwrite an original numeric fixture");
        }
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("NumericKeys")
                    .addColumn(new ColumnBuilder("ID", DataType.LONG))
                    .addColumn(new ColumnBuilder("Family", DataType.LONG))
                    .addColumn(new ColumnBuilder("Ordinal", DataType.LONG))
                    .addColumn(new ColumnBuilder("Kind", DataType.BOOLEAN))
                    .addColumn(new ColumnBuilder("SingleValue", DataType.FLOAT))
                    .addColumn(new ColumnBuilder("DoubleValue", DataType.DOUBLE))
                    .toTable(database);
            row(table, 1, 1, null, 1.0);
            row(table, 1, 2, 1.0f, null);
            row(table, 1, 3, null, 1.00000001);

            row(table, 2, 1, null, (double) Float.MIN_VALUE);
            row(table, 2, 2, Float.MIN_VALUE, null);
            row(table, 2, 3, null, 2e-45);

            row(table, 3, 1, 3e38f, null);
            row(table, 3, 2, null, 3e38);
            row(table, 3, 3, null, 1e300);

            row(table, 4, 1, -0.0f, null);
            row(table, 4, 2, null, 0.0);
            row(table, 4, 3, null, -0.0);
            database.flush();
        }
    }
}
