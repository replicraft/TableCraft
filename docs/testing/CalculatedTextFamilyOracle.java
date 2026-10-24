import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** Original read-only cached Text/property oracle; no expression evaluation or navigation. */
public class CalculatedTextFamilyOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected original path");
        }
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            List<String> names = new ArrayList<>(database.getTableNames());
            names.sort(String::compareTo);
            for (String name : names) {
                Table table = database.getTable(name);
                Column cached = table.getColumn("Cached");
                Object result = cached.getProperties().getValue("ResultType");
                Object format = cached.getProperties().getValue("TextFormat");
                if (!(result instanceof Byte) || (format != null && !(format instanceof Byte))) {
                    throw new AssertionError("Expected original BYTE calculation/property declarations");
                }
                System.out.printf("D\t%s\t%s\t%d\t%s\t%s\t%s\t%d\t%s\t%s%n",
                    name, cached.getType(), Short.toUnsignedInt(cached.getLength()),
                    cached.isCalculated(), cached.isCompressedUnicode(), cached.isHyperlink(),
                    Byte.toUnsignedInt((Byte) result), text((String) cached.getProperties().getValue("Expression")),
                    format == null ? "N" : Byte.toUnsignedInt((Byte) format));
                int ordinal = 0;
                for (Row row : table) {
                    System.out.printf("R\t%s\t%d\t%s\t%s%n", name, ordinal++,
                        row.get("ID"), text((String) row.get("Cached")));
                }
            }
        }
    }

    private static String text(String value) {
        return value == null ? "N" : "T" + HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }
}
