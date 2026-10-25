import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Row;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Locale;

/** Original bounded-row public-builder recipe and inert reopened getter oracle. */
public class LargeDeploymentFixture {
    private static final int ROWS = 1100;
    private static final int MEMO_UNITS = 32768;

    private static String memo(int identity) {
        char[] units = new char[MEMO_UNITS];
        for (int position = 0; position < units.length; position++) {
            units[position] = (char) (32 + (identity * 17 + position * 29 + position / 97) % 95);
        }
        return new String(units);
    }

    private static String hash(byte[] bytes) throws Exception {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    private static void inspect(Path path) throws Exception {
        MessageDigest allMemos = MessageDigest.getInstance("SHA-256");
        long identities = 0;
        long scores = 0;
        long memoUnits = 0;
        int count = 0;
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            Table table = database.getTable("LargeDeployment");
            System.out.println("Columns\t" + String.join("\t",
                table.getColumns().stream().map(column -> column.getName()).toList()));
            for (Row row : table) {
                int identity = row.getInt("Id");
                int score = row.getInt("Score");
                String label = row.getString("Label");
                String value = row.getString("Memo");
                if (identity != count + 1 || score != identity * 17 - 9
                        || !label.equals(String.format(Locale.ROOT, "row-%06d", identity))
                        || !value.equals(memo(identity))) {
                    throw new IllegalStateException("Reopened public getters differ from authored rows");
                }
                byte[] encoded = value.getBytes(StandardCharsets.UTF_8);
                allMemos.update(encoded);
                identities += identity;
                scores += score;
                memoUnits += value.length();
                count++;
                if (identity == 1 || identity == ROWS / 2 || identity == ROWS) {
                    System.out.printf(Locale.ROOT, "Row\t%d\t%d\t%s\t%d\t%s%n",
                        identity, score, label, value.length(), hash(encoded));
                }
            }
            if (count != ROWS || table.getRowCount() != ROWS) {
                throw new IllegalStateException("Reopened row counts differ");
            }
        }
        System.out.printf(Locale.ROOT, "Summary\t%d\t%d\t%d\t%d\t%s%n",
            count, identities, scores, memoUnits, HexFormat.of().formatHex(allMemos.digest()));
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected fresh path/profile or --inspect/path");
        }
        if (args[0].equals("--inspect")) {
            inspect(Path.of(args[1]));
            return;
        }

        Path path = Path.of(args[0]);
        if (Files.exists(path) || !(args[1].equals("V2016") || args[1].equals("V2019"))) {
            throw new IllegalArgumentException("Expected new path and ACE 16-family profile");
        }
        try (Database database = new DatabaseBuilder(path)
                .setFileFormat(Database.FileFormat.valueOf(args[1])).setAutoSync(false).create()) {
            database.setEvaluateExpressions(false);
            Table table = new TableBuilder("LargeDeployment")
                .addColumn(new ColumnBuilder("Id", DataType.LONG))
                .addColumn(new ColumnBuilder("Score", DataType.LONG))
                .addColumn(new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(32))
                .addColumn(new ColumnBuilder("Memo", DataType.MEMO).setCompressedUnicode(false))
                .toTable(database);
            for (int identity = 1; identity <= ROWS; identity++) {
                table.addRow(identity, identity * 17 - 9,
                    String.format(Locale.ROOT, "row-%06d", identity), memo(identity));
            }
            database.flush();
        }
        if (Files.size(path) <= 64L * 1024 * 1024) {
            throw new IllegalStateException("Original must exceed the prior 64 MiB image envelope");
        }
        System.out.println("Created\t" + Files.size(path));
    }
}
