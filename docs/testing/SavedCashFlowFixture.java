import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.nio.file.Path;

/** Original inert cashflow query metadata; no formula, SQL or VBA executes. */
public class SavedCashFlowFixture extends SavedStatisticsFixture {
    static void output(int id, String expression, String name, int order) throws Exception {
        row(id, (byte) 6, expression, null, name, null, order);
    }

    static void scalar(String name, String expression) throws Exception {
        output(definition(name, 0, 1), expression, "V", 1);
    }

    public static void main(String[] args) throws Exception {
        SavedForestFixture.main(args);

        try (Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
            db.setEvaluateExpressions(false);
            db.setAllowAutoNumberInsert(true);
            catalog = db.getSystemTable("MSysObjects");
            queries = db.getSystemTable("MSysQueries");

            scalar("CashNpv", "NPV(.1,Array(-100,0,121))");
            scalar("CashIrr", "IRR(Array(-100,0,121),)");
            scalar("CashMirr", "MIRR(Array(-100,0,121),0,0)");

            int id = definition("CashParams", 0, 1);
            row(id, (byte) 2, null, (short) 4, "P", null, 1);
            output(id, "NPV(0,Array([P],1))", "V", 1);

            id = definition("CashRows", 0, 1);
            from(id);
            output(id, "ID", "ID", 1);
            output(id, "NPV(0,Array(ID,1))", "V", 2);
            row(id, (byte) 11, "ID", null, "A", null, 1);

            id = definition("CashGrouped", 0, 1);
            from(id);
            output(id, "ID Mod 2", "G", 1);
            output(id, "Sum(NPV(0,Array(ID,1)))", "V", 2);
            row(id, (byte) 9, "ID Mod 2", null, null, null, 1);
            row(id, (byte) 11, "G", null, "A", null, 1);

            id = definition("CashCrosstab", 0x10, 6);
            from(id);
            row(id, (byte) 6, "ID Mod 2", (short) 2, "G", null, 1);
            row(id, (byte) 6, "Sum(NPV(0,Array(ID,1)))", (short) 2, "Total", null, 2);
            row(id, (byte) 6, "Sum(NPV(0,Array(ID,1)))", (short) 0, "Measure", null, 3);
            row(id, (byte) 6, "ID IN (1,2,4)", (short) 1, null, null, 4);
            row(id, (byte) 9, "ID Mod 2", (short) 2, null, null, 1);
            row(id, (byte) 11, "G", null, "A", null, 1);

            id = definition("CashDerived", 0, 1);
            row(id, (byte) 5, "SELECT NPV(0,Array(2,3)) AS V", null, null, "d", 1);
            output(id, "d.V", "V", 1);

            id = definition("CashUnion", 0x80, 9);
            row(id, (byte) 5, "SELECT NPV(0,Array(2,3)) AS V", null, null, "X7YZ_____1", 1);
            row(id, (byte) 5, "SELECT IRR(Array(-100,121)) AS V", null, null, "X7YZ_____2", 2);

            scalar("CashEmpty", "NPV(0,Array())");
            scalar("CashBadGuess", "IRR(Array(-100,121),1e100)");

            id = definition("CashLate", 0, 1);
            from(id);
            output(id, "NPV(0,Array(ID,1/(ID-2)))", "V", 1);

            db.flush();
            if (db.getQueries().size() != 26) {
                throw new AssertionError("Query count changed");
            }
        }

        System.out.println("Original cashflow query definitions created without execution.");
    }
}
