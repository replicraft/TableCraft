import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import java.nio.file.Path;

/** Original inert type-inspection metadata; no SQL, formula or VBA executes. */
public class SavedInspectionFixture extends SavedStatisticsFixture {
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

            int id = definition("MetaScalars", 0, 1);
            output(id, "TypeName(CInt(1))", "T", 1);
            output(id, "VarType(CInt(1))", "V", 2);
            output(id, "IsArray(CInt(1))", "A", 3);
            output(id, "IsEmpty(Null)", "E", 4);
            output(id, "IsError(1)", "F", 5);

            for (String name : new String[] {"MetaArray", "MetaEmptyArray"}) {
                String array = name.equals("MetaArray") ? "Array(1,Null,3)" : "VBA.Array()";
                id = definition(name, 0, 1);
                output(id, "TypeName(" + array + ")", "T", 1);
                output(id, "VarType(" + array + ")", "V", 2);
                output(id, "IsArray(" + array + ")", "A", 3);
                output(id, "LBound(" + array + ")", "L", 4);
                output(id, "UBound(" + array + ")", "U", 5);
            }

            id = definition("MetaRows", 0, 1);
            from(id);
            output(id, "ID", "ID", 1);
            output(id, "TypeName(ID)", "T", 2);
            output(id, "VarType(ID)", "V", 3);
            output(id, "LBound(Array(ID),)", "L", 4);
            output(id, "UBound(Array(ID,Null))", "U", 5);
            row(id, (byte) 11, "ID", null, "A", null, 1);

            id = definition("MetaParams", 0, 1);
            row(id, (byte) 2, null, (short) 4, "P", null, 1);
            output(id, "TypeName([P])", "T", 1);
            output(id, "VarType([P])", "V", 2);

            id = definition("MetaGrouped", 0, 1);
            from(id);
            output(id, "ID Mod 2", "G", 1);
            output(id, "Max(UBound(Array(ID,Null)))", "V", 2);
            row(id, (byte) 9, "ID Mod 2", null, null, null, 1);
            row(id, (byte) 11, "G", null, "A", null, 1);

            id = definition("MetaCrosstab", 0x10, 6);
            from(id);
            row(id, (byte) 6, "ID Mod 2", (short) 2, "G", null, 1);
            row(id, (byte) 6, "Sum(UBound(Array(ID,Null)))", (short) 2, "Total", null, 2);
            row(id, (byte) 6, "Sum(UBound(Array(ID,Null)))", (short) 0, "Measure", null, 3);
            row(id, (byte) 6, "ID IN (1,2,4)", (short) 1, null, null, 4);
            row(id, (byte) 9, "ID Mod 2", (short) 2, null, null, 1);
            row(id, (byte) 11, "G", null, "A", null, 1);

            id = definition("MetaDerived", 0, 1);
            row(id, (byte) 5, "SELECT VarType(Array(1)) AS V", null, null, "d", 1);
            output(id, "d.V", "V", 1);

            id = definition("MetaUnion", 0x80, 9);
            row(id, (byte) 5, "SELECT TypeName(CInt(1)) AS V", null, null, "X7YZ_____1", 1);
            row(id, (byte) 5, "SELECT TypeName(CDbl(1)) AS V", null, null, "X7YZ_____2", 2);

            scalar("MetaBadDimension", "UBound(Array(1),2)");
            scalar("MetaError", "IsError(1/0)");
            id = definition("MetaLate", 0, 1);
            from(id);
            output(id, "IsArray(Array(ID,1/(ID-2)))", "V", 1);

            db.flush();
            if (db.getQueries().size() != 26) {
                throw new AssertionError("Query count changed");
            }
        }

        System.out.println("Original inspection definitions created without execution.");
    }
}
