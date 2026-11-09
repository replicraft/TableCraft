import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original public linked-table addition on a fresh explicit frontend copy. */
public class LinkedQueryCaseFixture {
    public static void main(String[] arguments) throws Exception {
        Path original = Path.of(arguments[0]);
        Path copy = Path.of(arguments[1]);
        Files.copy(original, copy);
        try (Database database = new DatabaseBuilder(copy).open()) {
            database.setEvaluateExpressions(false);
            database.setLinkResolver((ignored, target) -> {
                throw new IOException("Authoring a link may not resolve its target");
            });
            database.createLinkedTable("Lowercase link", arguments[2], "remote people");
        }
    }
}
