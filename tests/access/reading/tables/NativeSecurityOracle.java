import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.crypt.CryptCodecProvider;
import com.healthmarketscience.jackcess.crypt.impl.SidRemasker;
import com.healthmarketscience.jackcess.impl.DatabaseImpl;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.HexFormat;
import java.util.Objects;
import javax.crypto.Cipher;
import javax.crypto.spec.SecretKeySpec;

/** Original read-only SID/password comparison; uses the independently reviewed reference fold. */
class NativeSecurityOracle {
    public static void main(String[] args) throws Exception {
        java.security.Security.addProvider(new org.bouncycastle.jce.provider.BouncyCastleProvider());
        String password = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8)).readLine();
        try (Database database = new DatabaseBuilder(new File(args[0])).setReadOnly(true)
                .setCodecProvider(new CryptCodecProvider(password)).open()) {
            database.setEvaluateExpressions(false);
            var db = (DatabaseImpl) database;
            var channel = db.getPageChannel();
            var page = channel.createPageBuffer();
            channel.readRootPage(page);
            int folded = SidRemasker.createSidKey(page, db.getFormat());
            byte[] key = java.nio.ByteBuffer.allocate(4).order(java.nio.ByteOrder.LITTLE_ENDIAN).putInt(folded).array();
            if (args[0].endsWith(".mdb")) {
                if (!Objects.equals(password, db.getDatabasePassword())) throw new IllegalStateException("Legacy password mismatch");
            } else if (page.getInt(db.getFormat().OFFSET_ENCODING_KEY) == 0 && db.getDatabasePassword() != null) {
                throw new IllegalStateException("Plain ACCDB still has a native password field");
            }
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            for (String[] binding : new String[][]{{"MSysACEs", "SID"}, {"MSysObjects", "Owner"}}) {
                var table = db.getSystemTable(binding[0]);
                int ordinal = 0;
                for (var row : table) {
                    byte[] sid = (byte[]) row.get(binding[1]);
                    String result = "NULL";
                    if (sid != null) {
                        Cipher cipher = Cipher.getInstance("ARCFOUR", "BC");
                        cipher.init(Cipher.DECRYPT_MODE, new SecretKeySpec(key, "ARCFOUR"));
                        result = HexFormat.of().formatHex(digest.digest(cipher.doFinal(sid)));
                    }
                    System.out.printf("%s\t%d\t%s%n", binding[0], ordinal++, result);
                }
            }
        }
    }
}
