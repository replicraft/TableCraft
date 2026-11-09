import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.nio.file.Path;

/** Original staged-schema definitions; no SQL or active database content executes. */
public class DiscoveryCrosstabFixture extends ComputedCrosstabFixture {
    public static void main(String[] args) throws Exception {
        ComputedCrosstabFixture.main(args);
        try (Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
            db.setEvaluateExpressions(false);
            db.setAllowAutoNumberInsert(true);
            catalog = db.getSystemTable("MSysObjects");
            queries = db.getSystemTable("MSysQueries");
            int id = cross("DiscoveredParameter", "Sales", "", "[Quarter] IN (CLng([P]))",
                    "[Quarter]", (short) 0, null, "Measure");
            row(id, (byte) 2, null, (short) 10, "P", null, 1);

            id = start("DiscoveredNested", true, (short) 0, null);
            row(id, (byte) 5, null, null, "CrossDynamic", null, 1);
            row(id, (byte) 6, "[Region]", (short) 2, "Region", null, 1);
            row(id, (byte) 6, "Sum([2])", (short) 0, "Measure", null, 2);
            row(id, (byte) 6, "[Region]", (short) 1, null, null, 3);
            row(id, (byte) 9, "[Region]", (short) 2, null, null, 1);
            row(id, (byte) 9, "[Region]", (short) 1, null, null, 2);
            row(id, (byte) 11, "[Region]", null, "A", null, 1);

            cross("DiscoveredVolatile", "Sales", "", "[Quarter]", "[Quarter]",
                    (short) 0, null, "Measure", "First(Rnd())");
            cross("DiscoveredErrors", "Sales", "", "[Quarter]", "[Quarter]",
                    (short) 0, null, "Measure", "First(CVErr(2001))");
            cross("DiscoveredCycle", "DiscoveredCycle", "", "[Region]", "[Region]",
                    (short) 0, null, "Measure", "Count(*)");
            db.flush();
            if (db.getQueries().size() != 47) {
                throw new AssertionError("Discovery query count changed");
            }
        }
        System.out.println("Original discovery crosstabs created; no stored query executed.");
    }
}
