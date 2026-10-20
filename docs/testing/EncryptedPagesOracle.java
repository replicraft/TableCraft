import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.crypt.CryptCodecProvider;
import com.healthmarketscience.jackcess.impl.DatabaseImpl;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.File;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HexFormat;

/** Original read-only reference driver; a single-line password comes from stdin. */
class EncryptedPagesOracle {
    public static void main(String[] args) throws Exception {
        String password = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8)).readLine();
        try (Database db = new DatabaseBuilder(new File(args[0])).setReadOnly(true)
                .setCodecProvider(new CryptCodecProvider(password)).open()) {
            db.setEvaluateExpressions(false);
            if (args.length == 3 && args[1].equals("--rows")) {
                var table = db.getTable(args[2]);
                StringBuilder output = new StringBuilder("H");
                for (var column : table.getColumns()) output.append('\t').append(column.getName());
                output.append('\n');
                for (var row : table) {
                    output.append("R");
                    for (var column : table.getColumns()) {
                        Object value = row.get(column.getName());
                        output.append('\t');
                        if (value == null) output.append("N");
                        else if (value instanceof Integer) output.append("I32:").append(value);
                        else if (value instanceof String) output.append("T").append(HexFormat.of().formatHex(((String)value).getBytes(StandardCharsets.UTF_8)));
                        else throw new IllegalArgumentException("Unqualified comparison carrier");
                    }
                    output.append('\n');
                }
                System.out.print(output);
                return;
            }
            var channel = ((DatabaseImpl) db).getPageChannel();
            int pages = Math.toIntExact(new File(args[0]).length() / channel.getFormat().PAGE_SIZE);
            ByteBuffer buffer = channel.createPageBuffer();
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            StringBuilder output = new StringBuilder();
            for (int page = 1; page < pages; ++page) {
                channel.readPage(buffer, page);
                output.append(page).append('\t').append(HexFormat.of().formatHex(digest.digest(buffer.array()))).append('\n');
            }
            System.out.print(output);
        }
    }
}
