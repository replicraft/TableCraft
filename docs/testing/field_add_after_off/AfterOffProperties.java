import com.healthmarketscience.jackcess.Column;
import com.healthmarketscience.jackcess.DataType;
import com.healthmarketscience.jackcess.Database;
import com.healthmarketscience.jackcess.DatabaseBuilder;
import com.healthmarketscience.jackcess.PropertyMap;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

// First-party inert observation; no SDK source or database template is copied.
public class AfterOffProperties {
    public static void main(String[] args) throws Exception {
        if (args.length != 3) {
            throw new IllegalArgumentException("Expected file, column and LONG/TEXT");
        }
        Path path = Path.of(args[0]);
        byte[] before = Files.readAllBytes(path);
        try (Database database = new DatabaseBuilder(path).setReadOnly(true).open()) {
            database.setEvaluateExpressions(false);
            PropertyMap settings = database.getDatabaseProperties();
            for (String name : new String[] {"Track Name AutoCorrect Info", "Perform Name AutoCorrect"}) {
                PropertyMap.Property property = settings.get(name);
                if (property == null || property.getType() != DataType.LONG
                    || !property.isDdl() || !Integer.valueOf(0).equals(property.getValue())) {
                    throw new IllegalStateException("Stored explicit OFF changed");
                }
                System.out.println("DatabaseProperty\t" + name + "\tLONG\tDDL\t0");
            }
            Column column = database.getTable("Table1").getColumn(args[1]);
            PropertyMap properties = column.getProperties();
            if (!Boolean.FALSE.equals(properties.getValue("Required"))) {
                throw new IllegalStateException("Required false changed");
            }
            if (args[2].equals("LONG")) {
                if (column.getType() != DataType.LONG
                    || !"0".equals(properties.getValue("DefaultValue"))) {
                    throw new IllegalStateException("Number declaration differs");
                }
            } else if (args[2].equals("TEXT")) {
                if (column.getType() != DataType.TEXT || column.getLengthInUnits() != 255
                    || !column.isCompressedUnicode()
                    || !Boolean.TRUE.equals(properties.getValue("AllowZeroLength"))
                    || properties.get("DefaultValue") != null) {
                    throw new IllegalStateException("Short Text declaration differs");
                }
            } else {
                throw new IllegalArgumentException("Unknown declaration");
            }
            System.out.println("AddedDeclaration\t" + column.getName() + "\t" + column.getType());
        }
        if (!Arrays.equals(before, Files.readAllBytes(path))) {
            throw new IllegalStateException("Read-only observation changed bytes");
        }
    }
}
