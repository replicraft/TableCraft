import com.healthmarketscience.jackcess.ColumnBuilder;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.InvalidValueException;
import com.healthmarketscience.jackcess.JackcessException;
import com.healthmarketscience.jackcess.Table;
import com.healthmarketscience.jackcess.TableBuilder;
import java.nio.file.Files;
import java.nio.file.Path;

/** Original pure-rule reference checks plus inert unsupported/invalid stored states. */
public class ValidationTextCallFixture {
    private static boolean referenceChecks;
    private static Table table(Database database, String name, String rule, boolean field) throws Exception {
        ColumnBuilder label = new ColumnBuilder("Label", DataType.TEXT).setLengthInUnits(255)
            .putProperty("AllowZeroLength", true);
        TableBuilder builder = new TableBuilder(name)
            .addColumn(new ColumnBuilder("ID", DataType.LONG));
        if (field) {
            label.putProperty("ValidationRule", rule);
        } else {
            builder.putProperty("ValidationRule", rule);
        }
        return builder.addColumn(label)
            .addColumn(new ColumnBuilder("Other", DataType.TEXT).setLengthInUnits(255)
                .putProperty("AllowZeroLength", true))
            .toTable(database);
    }

    private static int units(String value) {
        return value.length();
    }

    private static void checkedInsert(
        Table table, int identity, String left, String right, boolean accepted
    ) throws Exception {
        if (!referenceChecks) {
            if (accepted) {
                table.addRow(identity, left, right);
            }
            return;
        }
        try {
            table.addRow(identity, left, right);
            if (!accepted) {
                throw new AssertionError("Original false rule unexpectedly accepted: " + table.getName());
            }
        } catch (InvalidValueException rejected) {
            if (accepted) {
                throw rejected;
            }
        }
    }

    private static void unknownNull(Table table) throws Exception {
        if (!referenceChecks) {
            return;
        }
        try {
            table.addRow(-1, null, null);
        } catch (JackcessException uncertain) {
            if (uncertain.getMessage().startsWith("Condition evaluated to Null")) {
                return;
            }
            throw uncertain;
        }
        throw new AssertionError("Expected distinct original NULL rule outcome");
    }

    private static void attach(Table table, String rule, boolean field) throws Exception {
        var properties = field ? table.getColumn("Label").getProperties() : table.getProperties();
        properties.put("ValidationRule", rule);
        properties.save();
    }

    public static void main(String[] args) throws Exception {
        if (args.length < 2 || args.length > 3) {
            throw new IllegalArgumentException("Expected fresh path, V2016/V2019 and optional --reference-check");
        }
        referenceChecks = args.length == 3 && args[2].equals("--reference-check");
        if (args.length == 3 && !referenceChecks) {
            throw new IllegalArgumentException("Unknown fixture mode");
        }
        Path path = Path.of(args[0]);
        if (Files.exists(path)) {
            throw new IllegalArgumentException("Output must be fresh");
        }
        Database.FileFormat format = switch (args[1]) {
            case "V2016" -> Database.FileFormat.V2016;
            case "V2019" -> Database.FileFormat.V2019;
            default -> throw new IllegalArgumentException("Unsupported profile");
        };
        try (Database database = new DatabaseBuilder(path).setFileFormat(format).create()) {
            // Normal corpus construction is entirely inert. The separate explicit
            // probe mode executes only these original supplied pure rules.
            database.setEvaluateExpressions(referenceChecks);
            String[] values = {
                "", "ABC", "abc", "a".repeat(99), "a".repeat(100),
                "🦀".repeat(49), "🦀".repeat(50), "a" + "🦀".repeat(49),
                "é".repeat(49) + "b", "\uE000".repeat(99), "a\u0000b", "\t\"\\\n",
            };
            for (boolean field : new boolean[] {true, false}) {
                String prefix = field ? "Field" : "Table";
                String lengthRule = "Len([Label])<100";
                Table length = table(database, prefix + "Len", lengthRule, field);
                Table nullable = table(database, prefix + "LenNullable",
                    "[Label] Is Null Or Len([Label])<100", field);
                Table binary = table(database, prefix + "Binary",
                    "StrComp([Label],\"ABC\",0)=0", field);
                table(database, prefix + "LenEmpty", lengthRule, field);
                for (int index = 0; index < values.length; index++) {
                    String value = values[index];
                    checkedInsert(length, index, value, "unused", units(value) < 100);
                    checkedInsert(nullable, index, value, "unused", units(value) < 100);
                    checkedInsert(binary, index, value, "unused", value.equals("ABC"));
                }
                nullable.addRow(values.length, null, null);
                unknownNull(length);
                unknownNull(binary);
            }
            Table pairs = table(database, "BinaryPairs", "StrComp([Label],[Other],0)<=0", false);
            Table nullablePairs = table(database, "BinaryPairsNullable",
                "[Label] Is Null Or [Other] Is Null Or StrComp([Label],[Other],0)<=0", false);
            String[][] inputs = {
                {"", ""}, {"A", "a"}, {"a", "A"}, {"abc", "abc"},
                {"🦀", "\uE000"}, {"\uE000", "🦀"}, {"é", "é"}, {"é", "é"},
                {"a-", "a "}, {"a ", "a-"}, {"x", "x "}, {"x\u0000", "x"},
                {"a\u0000", "a\u0001"}, {"\t", "\n"}, {"🦀", "🦁"},
                {"ÿ", "Ā"}, {"Ā", "ÿ"},
            };
            for (int index = 0; index < inputs.length; index++) {
                String left = inputs[index][0];
                String right = inputs[index][1];
                boolean accepted = left.compareTo(right) <= 0;
                checkedInsert(pairs, index, left, right, accepted);
                checkedInsert(nullablePairs, index, left, right, accepted);
            }
            nullablePairs.addRow(inputs.length, null, "x");
            nullablePairs.addRow(inputs.length + 1, "x", null);
            nullablePairs.addRow(inputs.length + 2, null, null);
            unknownNull(pairs);
            if (referenceChecks) {
                System.out.println("Explicit pure-rule reference insert/rejection assertions passed");
                return;
            }

            // Remaining states store properties as inert data after insertion.
            // Unknown/locale/clock calls and invalid historical states never run.
            database.setEvaluateExpressions(false);
            for (boolean field : new boolean[] {true, false}) {
                String prefix = field ? "Field" : "Table";
                Table nullLength = table(database, prefix + "LenUnknownNull", null, field);
                nullLength.addRow(1, null, null);
                attach(nullLength, "Len([Label])<100", field);
                Table nullBinary = table(database, prefix + "BinaryUnknownNull", null, field);
                nullBinary.addRow(1, null, null);
                attach(nullBinary, "StrComp([Label],\"ABC\",0)=0", field);

                Table invalidLength = table(database, prefix + "LenInvalid", null, field);
                invalidLength.addRow(1, "ok", "unused");
                invalidLength.addRow(2, "a".repeat(100), "unused");
                attach(invalidLength, "Len([Label])<100", field);
                Table astralLength = table(database, prefix + "LenAstralInvalid", null, field);
                astralLength.addRow(1, "ok", "unused");
                astralLength.addRow(2, "🦀".repeat(50), "unused");
                attach(astralLength, "Len([Label])<100", field);
                Table nulLength = table(database, prefix + "LenNulInvalid", null, field);
                nulLength.addRow(1, "ok", "unused");
                nulLength.addRow(2, "a".repeat(98) + "\u0000b", "unused");
                attach(nulLength, "Len([Label])<100", field);
                Table invalidBinary = table(database, prefix + "BinaryInvalid", null, field);
                invalidBinary.addRow(1, "ABC", "unused");
                invalidBinary.addRow(2, "abc", "unused");
                attach(invalidBinary, "StrComp([Label],\"ABC\",0)=0", field);

                String[][] excluded = {
                    {"Locale", "StrComp([Label],\"ABC\",1)=0"},
                    {"MissingMode", "StrComp([Label],\"ABC\")=0"},
                    {"Upper", "UCase([Label])=\"ABC\""},
                    {"Clock", "Date() Is Not Null"},
                    {"Unknown", "UnknownFunction([Label])=0"},
                    {"Numeric", "Len([ID])<100"},
                    {"Foreign", "Len([Foreign].[Label])<100"},
                };
                for (String[] excludedRule : excluded) {
                    for (boolean empty : new boolean[] {false, true}) {
                        String name = prefix + excludedRule[0] + (empty ? "Empty" : "Stored");
                        Table unsupported = table(database, name, null, field);
                        if (!empty) {
                            unsupported.addRow(1, "ABC", "unused");
                        }
                        attach(unsupported, excludedRule[1], field);
                    }
                }
            }
            Table crossField = table(database, "FieldCrossField", null, true);
            attach(crossField, "Len([Other])<100", true);
        }
        System.out.println("Original validation-rule corpus written with expression evaluation disabled");
    }
}
