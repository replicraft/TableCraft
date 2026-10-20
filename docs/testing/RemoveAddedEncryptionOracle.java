import com.healthmarketscience.jackcess.crypt.EncryptionUtil;
import com.healthmarketscience.jackcess.crypt.PasswordUtil;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original experiment driver: rewrite only freshly generated test candidates. */
class RemoveAddedEncryptionOracle {
    public static void main(String[] args) throws Exception {
        java.security.Security.addProvider(new org.bouncycastle.jce.provider.BouncyCastleProvider());
        String password = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8)).readLine();
        Path source = Path.of(args[0]);
        Path output = Path.of(args[1]);
        if (Files.exists(output)) throw new IllegalStateException("Output must be fresh");
        byte[] header = new byte[21];
        try (var input = Files.newInputStream(source)) { input.readNBytes(header, 0, header.length); }
        if (header[20] == 1) {
            EncryptionUtil.removeJetEncoding(source, output);
            PasswordUtil.removeDatabasePassword(output);
        } else {
            EncryptionUtil.removeOfficeEncryption(source, output, password);
        }
    }
}
