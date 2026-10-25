import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.DatabaseImpl;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.Base64;

/** Original getter-only capture; no reference weights, SQL or expression execution. */
public final class ConcatenationPolicyOracle {
    private static String text(String value) {
        return value == null ? "N" : "T" + Base64.getEncoder().encodeToString(
            value.getBytes(StandardCharsets.UTF_8));
    }

    private static String order(ColumnImpl.SortOrder value) {
        return value.getValue() + "\t" + value.getVersion() + "\t" + value.getVariant();
    }

    public static void main(String[] args) throws Exception {
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((source, token) -> {
                throw new AssertionError("Getter may not resolve a linked target");
            });
            System.out.println("DATABASE\t" + order(((DatabaseImpl) database).getDefaultSortOrder()));

            var table = database.getTable("PolicyProbe");
            for (String name : new String[] {"LegacyText", "ModernText"}) {
                var column = (ColumnImpl) table.getColumn(name);
                System.out.println("COLUMN\t" + name + "\t" + column.getType()
                    + "\t" + order(column.getTextSortOrder()));
            }
            for (Row row : table) {
                System.out.println("ROW\t" + row.getInt("ID") + "\t" + text(row.getString("LegacyText"))
                    + "\t" + text(row.getString("ModernText")));
            }
            if (!database.getLinkedDatabases().isEmpty()) {
                throw new AssertionError("Unexpected linked handles");
            }
        }
    }
}
