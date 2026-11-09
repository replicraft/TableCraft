import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import java.math.BigDecimal;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original inert cached Numeric fixture and exact public getter/write oracle. */
public class CalculatedDecimalFixture {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019, or --inspect path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
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
            Table table = builder("CalculatedDecimals").toTable(database);
            String[] values = {
                "0", "0.0000", "-1.2345", "123.450000",
                "9999999999999999999999999999", "-9999999999999999999999999999",
                "0.0000000000000000000000000001", "-0.0000000000000000000000000001",
                "0.9999999999999999999999999999", "-0.9999999999999999999999999999"
            };
            int identity = 1;
            for (String value : values) {
                table.addRow(identity++, new BigDecimal(value));
            }
            table.addRow(identity, null);
            builder("EmptyCalculatedDecimals").toTable(database);
        }
    }

    private static TableBuilder builder(String name) {
        return new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Amount", DataType.NUMERIC).setCalculatedInfo("[ID]/10000"));
    }

    private static void inspect(Path path) throws Exception {
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("CalculatedDecimals");
            ColumnImpl column = (ColumnImpl) table.getColumn("Amount");
            System.out.printf("D\t%s\t%d\t%d\t%d%n", column.getType(), column.getLength(), column.getPrecision(), column.getScale());
            int ordinal = 0;
            for (Row row : table) {
                Object value = row.get("Amount");
                if (value == null) {
                    System.out.printf("R\t%d\tN\tN\tN%n", ordinal++);
                    continue;
                }
                if (!(value instanceof BigDecimal decimal)) {
                    throw new IllegalStateException("Expected stored BigDecimal carrier");
                }
                var encoded = column.write(decimal, 0, ByteOrder.LITTLE_ENDIAN);
                byte[] bytes = new byte[encoded.remaining()];
                encoded.get(bytes);
                System.out.printf("R\t%d\t%s\t%d\t%s%n", ordinal++, decimal.unscaledValue(), decimal.scale(), HexFormat.of().formatHex(bytes));
            }
        }
    }
}
