import com.healthmarketscience.jackcess.*;
import java.math.*;
import java.nio.file.*;

/** Original inert ACE 16 fixed-point index recipe; no floating conversion. */
public class DecimalIndexFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action) throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static ColumnBuilder decimal(String name,int precision,int scale) {
    return new ColumnBuilder(name,DataType.NUMERIC).setPrecision(precision).setScale(scale);
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    String[] coefficients={"0","1","-1","10","-10","9007199254740993","9007199254740994","-9007199254740993",
      "9223372036854775807","9223372036854775808","-9223372036854775808","-9223372036854775809",
      "311917102708983781990730508","-311917102708983781990730508","9999999999999999999999999999","-9999999999999999999999999999"};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);
      if(((com.healthmarketscience.jackcess.impl.DatabaseImpl)db).getFormat().LEGACY_NUMERIC_INDEXES)
        throw new AssertionError("Expected corrected fixed-point index encoding");
      for(int scale:new int[]{0,4,28}) {
        Table table=new TableBuilder("Decimal"+scale).addColumn(new ColumnBuilder("ID",DataType.LONG))
          .addColumn(decimal("Value",28,scale))
          .addIndex(new IndexBuilder("DecimalAsc").addColumns("Value"))
          .addIndex(new IndexBuilder("DecimalDesc").addColumns(false,"Value")).toTable(db);
        for(int i=0;i<1000;i++)table.addRow(i,i%11==0?null:new BigDecimal(new BigInteger(coefficients[(i*7)%coefficients.length]),scale));
        for(boolean primary:new boolean[]{false,true}) {
          Table unique=new TableBuilder((primary?"DecimalPrimary":"DecimalUnique")+scale)
            .addColumn(decimal("Value",28,scale))
            .addIndex(primary?new IndexBuilder("DecimalKey").addColumns("Value").setPrimaryKey()
              :new IndexBuilder("DecimalKey").addColumns("Value").setUnique()).toTable(db);
          for(String coefficient:coefficients) {
            BigDecimal value=new BigDecimal(new BigInteger(coefficient),scale);unique.addRow(value);
            rejects(()->unique.addRow(value));
          }
          if(primary)rejects(()->unique.addRow((Object)null));
          else{unique.addRow((Object)null);unique.addRow((Object)null);}
        }
        new TableBuilder("DecimalEmpty"+scale).addColumn(decimal("Value",28,scale))
          .addIndex(new IndexBuilder("EmptyKey").addColumns("Value")).toTable(db);
      }
      Table small=new TableBuilder("DecimalSmall").addColumn(decimal("Value",4,2))
        .addIndex(new IndexBuilder("SmallAsc").addColumns("Value"))
        .addIndex(new IndexBuilder("SmallDesc").addColumns(false,"Value")).toTable(db);
      for(String coefficient:new String[]{"0","1","-1","9999","-9999"})small.addRow(new BigDecimal(new BigInteger(coefficient),2));
      small.addRow((Object)null);small.addRow((Object)null);
      new TableBuilder("DecimalCompound").addColumn(decimal("Value",28,4)).addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addIndex(new IndexBuilder("CompoundKey").addColumns("Value","ID")).toTable(db);
    }
    System.out.println("Exact 28-digit coefficients, scales, unique/primary and NULL assertions passed.");
  }
}
