import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.nio.file.Path;

/** Original computed-heading metadata; no stored SQL or active content is executed. */
public class ComputedCrosstabFixture extends SavedNamedCrosstabFixture {
    public static void main(String[] args) throws Exception {
        SavedNamedCrosstabFixture.main(args);
        try (Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
            db.setEvaluateExpressions(false);
            db.setAllowAutoNumberInsert(true);
            catalog = db.getSystemTable("MSysObjects");
            queries = db.getSystemTable("MSysQueries");

            cross("ComputedFixed", "Sales", "",
                    "[Quarter] IN (1+1,CLng(4/4),CInt(2^2))", "[Quarter]",
                    (short) 0, null, "Measure");
            select("ComputedSelect", "ComputedFixed", "P",
                    new String[]{"[P].[Region]", "[P].[2]"}, new String[]{"Region", "Two"});

            String pivot = "IIf([Quarter]=1,'Sales.Q1','Sales!Q2')";
            cross("ComputedText", "Sales", "",
                    pivot + " IN ('Sales'&'.Q1','Sales'+'!Q2')", pivot,
                    (short) 0, null, "Measure");
            select("ComputedTextSelect", "ComputedText", "P",
                    new String[]{"[P].[Region]", "[P].[Sales!Q2]"}, new String[]{"Region", "Two"});

            cross("ComputedCasts", "Sales", "",
                    "[Quarter] IN (CVar(Abs(-2)),CByte(Sgn(9)),CLng(Sqr(16)))", "[Quarter]",
                    (short) 0, null, "Measure");
            cross("ComputedError", "Sales", "", "[Quarter] IN (1/0)", "[Quarter]",
                    (short) 0, null, "Measure");
            cross("ComputedConflict", "Sales", "", "[Quarter] IN (1+1,2)", "[Quarter]",
                    (short) 0, null, "Measure");
            cross("ComputedContext", "Sales", "", "[Quarter] IN (CLng('2'))", "[Quarter]",
                    (short) 0, null, "Measure");
            cross("ComputedLateError", "Sales", "", "[Quarter] IN (1+0,1+1,1+2)", "[Quarter]",
                    (short) 0, null, "Measure", "Sum(1/([Quarter]-3))");
            db.flush();
            if (db.getQueries().size() != 42) {
                throw new AssertionError("Computed query count changed");
            }
        }
        System.out.println("Original computed fixed crosstabs created; no stored query executed.");
    }
}
