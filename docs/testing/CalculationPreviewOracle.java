import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Date;
import java.util.HexFormat;
import java.util.TimeZone;

/** Original read-only declaration/dependency oracle; never evaluates expressions. */
public class CalculationPreviewOracle {
    public static void main(String[] args) throws Exception {
        if (args.length < 3) {
            throw new IllegalArgumentException("Expected path, table, target and ordinary dependencies");
        }

        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setTimeZone(TimeZone.getTimeZone("UTC"));
            database.setDateTimeType(DateTimeType.DATE);
            Table table = database.getTable(args[1]);
            Column target = table.getColumn(args[2]);
            Object expression = target.getProperties().getValue("Expression");
            if (expression == null || !target.isCalculated()) {
                throw new IllegalArgumentException("Expected a stored calculation declaration");
            }
            System.out.println("D\t" + text(expression.toString()));

            for (int index = 3; index < args.length; index++) {
                if (table.getColumn(args[index]).isCalculated()) {
                    throw new IllegalArgumentException("Dependencies must be ordinary fields");
                }
            }

            int ordinal = 0;
            for (Row row : table) {
                StringBuilder output = new StringBuilder("R\t").append(ordinal++);
                output.append('\t').append(scalar(row.get(target.getName())));
                for (int index = 3; index < args.length; index++) {
                    output.append('\t').append(scalar(row.get(args[index])));
                }
                System.out.println(output);
            }
        }
    }

    private static String text(String value) {
        return "T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }

    private static String scalar(Object value) {
        if (value == null) {
            return "N";
        }
        if (value instanceof Integer integer) {
            return "I32:" + integer;
        }
        if (value instanceof String string) {
            return text(string);
        }
        if (value instanceof BigDecimal decimal) {
            return "Q" + decimal.unscaledValue() + ":" + decimal.scale();
        }
        if (value instanceof Double number) {
            return "D" + String.format("%016x", Double.doubleToRawLongBits(number));
        }
        if (value instanceof Date date) {
            return "Y" + date.getTime();
        }
        if (value instanceof Boolean flag) {
            return flag ? "B1" : "B0";
        }
        throw new IllegalArgumentException("Unexpected original getter carrier");
    }
}
