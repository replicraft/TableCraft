import com.healthmarketscience.jackcess.crypt.EncryptionUtil;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

// Original inert engineering driver. Only a fresh explicit output may change.
class AddEncryptionOracle {
    public static void main(String[] args) throws Exception {
        if (args.length != 2) {
            throw new IllegalArgumentException("Expected source and fresh output");
        }
        Path source = Path.of(args[0]);
        Path output = Path.of(args[1]);
        if (Files.exists(output)) {
            throw new IllegalArgumentException("Output must not exist");
        }
        String password = new BufferedReader(
            new InputStreamReader(System.in, StandardCharsets.UTF_8)
        ).readLine();
        if (password == null || password.isEmpty()) {
            throw new IllegalArgumentException("A public test password is required");
        }
        EncryptionUtil.addOfficeAgileEncryption(source, output, password);
    }
}
