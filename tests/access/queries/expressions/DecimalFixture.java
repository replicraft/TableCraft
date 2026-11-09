import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
import java.math.BigDecimal;
public class DecimalFixture {
  public static void main(String[] args) throws Exception {
    if(java.nio.file.Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Output must not exist");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.V2007).create()) {
      Table table=new TableBuilder("DecimalSamples")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Amount",DataType.NUMERIC).setPrecision(28).setScale(4))
        .addColumn(new ColumnBuilder("Tiny",DataType.NUMERIC).setPrecision(28).setScale(28))
        .toTable(db);
      table.addRow(1,new BigDecimal("999999999999999999999999.9999"),new BigDecimal("0.0000000000000000000000000001"));
      table.addRow(2,new BigDecimal("-9007199254740993.1200"),new BigDecimal("-0.9999999999999999999999999999"));
      table.addRow(3,new BigDecimal("0.0000"),new BigDecimal("0.0000000000000000000000000000"));
      table.addRow(4,null,null);
    }
  }
}
