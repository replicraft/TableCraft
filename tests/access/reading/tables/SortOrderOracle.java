import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.crypt.CryptCodecProvider;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.DatabaseImpl;
import java.io.BufferedReader;
import java.io.File;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;

/** Original inert metadata comparison driver, not a collation execution oracle. */
class SortOrderOracle {
    private static void order(String prefix, ColumnImpl.SortOrder order) {
        System.out.printf("%s\t%d\t%d\t%d%n", prefix, order.getValue() & 0xffff,
                order.getVariant() & 0xff, order.getVersion() & 0xff);
    }
    public static void main(String[] args) throws Exception {
        String password = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8)).readLine();
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true)
                .setCodecProvider(new CryptCodecProvider(password)).open()) {
            database.setEvaluateExpressions(false);
            order("D", ((DatabaseImpl) database).getDefaultSortOrder());
            var table = database.getTable(args[1]);
            for (var column : table.getColumns()) {
                if (column.getType().isTextual()) {
                    order("C\t" + HexFormat.of().formatHex(column.getName().getBytes(StandardCharsets.UTF_8)),
                            ((ColumnImpl) column).getTextSortOrder());
                }
            }
        }
    }
}
