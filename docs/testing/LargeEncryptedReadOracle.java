import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.crypt.CryptCodecProvider;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.HexFormat;

/** Original inert getter driver; credentials are supplied on stdin, never evaluated. */
class LargeEncryptedReadOracle {
    public static void main(String[] args) throws Exception {
        java.security.Security.addProvider(new org.bouncycastle.jce.provider.BouncyCastleProvider());
        String password = new BufferedReader(
            new InputStreamReader(System.in, StandardCharsets.UTF_8)
        ).readLine();
        MessageDigest memoDigest = MessageDigest.getInstance("SHA-256");
        MessageDigest allMemos = MessageDigest.getInstance("SHA-256");
        int count = 0;
        long identities = 0;
        long scores = 0;
        long units = 0;

        try (Database database = new DatabaseBuilder(Path.of(args[0]))
                .setReadOnly(true)
                .setCodecProvider(new CryptCodecProvider(password))
                .open()) {
            database.setEvaluateExpressions(false);
            var table = database.getTable("LargeDeployment");
            System.out.println("Columns\t" + String.join("\t",
                table.getColumns().stream().map(column -> column.getName()).toList()));

            for (var row : table) {
                int identity = row.getInt("Id");
                int score = row.getInt("Score");
                String label = row.getString("Label");
                String memo = row.getString("Memo");
                byte[] bytes = memo.getBytes(StandardCharsets.UTF_8);
                allMemos.update(bytes);
                System.out.printf("Row\t%d\t%d\t%s\t%d\t%s%n",
                    identity, score, label, memo.length(),
                    HexFormat.of().formatHex(memoDigest.digest(bytes)));
                count++;
                identities += identity;
                scores += score;
                units += memo.length();
            }

            if (count != table.getRowCount()) {
                throw new IllegalStateException("Declared and enumerated row counts differ");
            }
        }

        System.out.printf("Summary\t%d\t%d\t%d\t%d\t%s%n",
            count, identities, scores, units,
            HexFormat.of().formatHex(allMemos.digest()));
    }
}
