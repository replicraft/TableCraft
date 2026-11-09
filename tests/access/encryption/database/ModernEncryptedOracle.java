import com.healthmarketscience.jackcess.Cursor;
import com.healthmarketscience.jackcess.CursorBuilder;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.DateTimeType;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.crypt.CryptCodecProvider;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;

// Original read-only driver. A single public test password arrives through stdin.
class ModernEncryptedOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 2 && args.length != 3) {
            throw new IllegalArgumentException("Expected database, table and optional index");
        }
        String password = new BufferedReader(
            new InputStreamReader(System.in, StandardCharsets.UTF_8)
        ).readLine();
        try (Database database = new DatabaseBuilder(Path.of(args[0])).setReadOnly(true)
                .setCodecProvider(new CryptCodecProvider(password)).open()) {
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            Table table = database.getTable(args[1]);
            Cursor cursor = args.length == 3
                ? CursorBuilder.createCursor(table.getIndex(args[2]))
                : CursorBuilder.createCursor(table);
            TableDataOracle.rows(args[1], table, cursor, false);
        }
    }
}
