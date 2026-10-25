import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import java.io.IOException;
import java.nio.file.Path;
import java.util.concurrent.atomic.AtomicInteger;

/** Original raw-cell getter oracle; no reference SQL or expression evaluation. */
public class NumericKeyOracle {
    public static void main(String[] arguments) throws Exception {
        AtomicInteger opens = new AtomicInteger();
        try (Database database = new DatabaseBuilder(Path.of(arguments[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((ignored, token) -> {
                opens.incrementAndGet();
                throw new IOException("Numeric getter may not resolve links");
            });
            var table = database.getTable("NumericKeys");
            for (var column : table.getColumns()) {
                System.out.printf("C\t%s\t%s%n", column.getName(), column.getType());
            }
            for (Row row : table) {
                Float single = row.getFloat("SingleValue");
                Double real = row.getDouble("DoubleValue");
                String singleBits = single == null ? "N" : String.format("S%08x", Float.floatToRawIntBits(single));
                String doubleBits = real == null ? "N" : String.format("D%016x", Double.doubleToRawLongBits(real));
                System.out.printf("R\t%d\t%d\t%d\t%b\t%s\t%s%n", row.getInt("ID"), row.getInt("Family"),
                        row.getInt("Ordinal"), row.getBoolean("Kind"), singleBits, doubleBits);
            }
        }
        if (opens.get() != 0) {
            throw new IllegalStateException("Numeric getter opened another database");
        }
        System.out.println("O\t0");
    }
}
