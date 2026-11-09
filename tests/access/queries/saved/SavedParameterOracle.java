import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.query.Query;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HexFormat;

/** Original inert parameter metadata getter; never renders or executes SQL. */
public class SavedParameterOracle {
    private static String hex(String value) {
        return HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            throw new IllegalArgumentException("Expected one database path");
        }

        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            for (Query query : database.getQueries()) {
                System.out.println("QUERY\t" + hex(query.getName()) + "\t" + query.getType());
                for (String parameter : query.getParameters()) {
                    System.out.println("PARAMETER\t" + hex(parameter));
                }
            }
        }
    }
}
