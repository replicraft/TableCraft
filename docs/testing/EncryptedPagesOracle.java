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
