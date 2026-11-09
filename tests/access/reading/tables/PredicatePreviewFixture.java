import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

/** Original stored predicate declarations and deliberately stale Boolean caches. */
public class PredicatePreviewFixture {
    public static void main(String[] arguments) throws Exception {
        if (arguments.length != 2) {
            throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
        }
        Path path = Path.of(arguments[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Fresh path required");
        }
        Database.FileFormat format = switch (arguments[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            database.setEvaluateExpressions(false);
            Map<String, String> declarations = declarations();
            Table table = builder("Predicates", declarations).toTable(database);
            builder("EmptyPredicates", declarations).toTable(database);
            Integer[] inputs = {-2, -1, 0, 1, 2, null};
            for (int ordinal = 0; ordinal < inputs.length; ordinal++) {
                Object[] row = new Object[declarations.size() + 2];
                row[0] = ordinal + 1;
                row[1] = inputs[ordinal];
                int position = 2;
                for (String name : declarations.keySet()) {
                    Boolean modeled = model(name, inputs[ordinal]);
                    row[position++] = modeled == null || !modeled;
                }
                table.addRow(row);
            }
        }
    }

    private static TableBuilder builder(String name, Map<String, String> declarations) {
        TableBuilder table = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG))
            .addColumn(new ColumnBuilder("Input", DataType.LONG));
        declarations.forEach((field, expression) -> table.addColumn(
            new ColumnBuilder(field, DataType.BOOLEAN).setCalculatedInfo(expression)));
        return table;
    }

    private static Map<String, String> declarations() {
        Map<String, String> values = new LinkedHashMap<>();
        values.put("Equal", "[Input]=0");
        values.put("NotEqual", "[Input]<>0");
        values.put("Less", "[Input]<0");
        values.put("LessEqual", "[Input]<=0");
        values.put("Greater", "[Input]>0");
        values.put("GreaterEqual", "[Input]>=0");
        values.put("Negated", "Not ([Input]=0)");
        values.put("Conjunction", "([Input]>-1) And ([Input]<2)");
        values.put("Disjunction", "([Input]<0) Or ([Input]>1)");
        values.put("Exclusive", "([Input]>=0) Xor ([Input]<=0)");
        values.put("Equivalent", "([Input]>=0) Eqv ([Input]<=0)");
        values.put("Implies", "([Input]<0) Imp ([Input]>0)");
        values.put("Missing", "[Input] Is Null");
        values.put("Present", "[Input] Is Not Null");
        values.put("NegatedMissing", "Not ([Input] Is Null)");
        values.put("FalseAnd", "False And ([Input]=0)");
        values.put("TrueAnd", "True And ([Input]=0)");
        values.put("FalseOr", "False Or ([Input]=0)");
        values.put("TrueOr", "True Or ([Input]=0)");
        values.put("TrueXor", "True Xor ([Input]=0)");
        values.put("FalseEqv", "False Eqv ([Input]=0)");
        values.put("TrueConsequent", "([Input]=0) Imp True");
        values.put("FalseAntecedent", "False Imp ([Input]=0)");
        values.put("TrueAntecedent", "True Imp ([Input]=0)");
        values.put("FalseConsequent", "([Input]=0) Imp False");
        values.put("NotUnknown", "Not Null");
        values.put("BlockedCall", "Len('x')>0");
        values.put("BlockedDependency", "[Equal]=True");
        values.put("LateEagerError", "([Input]<>1) Or (1/([Input]-2)>0)");
        return values;
    }

    // Used only to author visibly stale caches. The read-only runner independently
    // computes its expectations from persisted ordinary getters, not these caches.
    private static Boolean model(String name, Integer value) {
        if (name.equals("Missing")) {
            return value == null;
        }
        if (name.equals("Present") || name.equals("NegatedMissing")) {
            return value != null;
        }
        if (name.equals("FalseAnd")) {
            return false;
        }
        if (name.equals("TrueOr") || name.equals("TrueConsequent") || name.equals("FalseAntecedent")) {
            return true;
        }
        if (name.equals("NotUnknown")) {
            return null;
        }
        if (value == null) {
            return null;
        }
        return switch (name) {
            case "Equal" -> value == 0;
            case "NotEqual", "Negated" -> value != 0;
            case "Less" -> value < 0;
            case "LessEqual" -> value <= 0;
            case "Greater" -> value > 0;
            case "GreaterEqual" -> value >= 0;
            case "Conjunction" -> value > -1 && value < 2;
            case "Disjunction" -> value < 0 || value > 1;
            case "Exclusive" -> (value >= 0) != (value <= 0);
            case "Equivalent" -> (value >= 0) == (value <= 0);
            case "Implies" -> !(value < 0) || value > 0;
            case "TrueAnd", "FalseOr", "TrueAntecedent" -> value == 0;
            case "TrueXor", "FalseEqv", "FalseConsequent" -> value != 0;
            default -> null;
        };
    }
}
