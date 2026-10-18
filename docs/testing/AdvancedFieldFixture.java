import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original repository-licensed metadata/child-schema recipe. Fresh outputs only.
public class AdvancedFieldFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019, or --inspect path");
    if(args[0].equals("--inspect")){inspect(Path.of(args[1]));return;}
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      new TableBuilder("Advanced")
        .addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true))
        .addColumn(new ColumnBuilder("Amount",DataType.LONG))
        .addColumn(new ColumnBuilder("Twice",DataType.LONG).setCalculatedInfo("[Amount]*2"))
        .addColumn(new ColumnBuilder("Choices").setMultiValue(DataType.TEXT).setValueList("Red", "Blue"))
        .addColumn(new ColumnBuilder("Files").setAttachment())
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO).setAppendOnly(true))
        .addColumn(new ColumnBuilder("Rich",DataType.MEMO).putProperty("TextFormat",DataType.BYTE,(byte)1))
        .addColumn(new ColumnBuilder("Link",DataType.MEMO).setHyperlink(true))
        .addColumn(new ColumnBuilder("Masked",DataType.TEXT).setLengthInUnits(20).putProperty("InputMask","0000"))
        .toTable(db);
    }
    inspect(path);
  }
  static String flag(boolean value){return value?"TRUE":"FALSE";}
  static String property(Column c,String key)throws Exception{
    Object value=c.getProperties().getValue(key);return value==null?"UNKNOWN":value instanceof Boolean b?flag(b):value.toString();
  }
  static void inspect(Path path)throws Exception{
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()){
      for(Column c:db.getTable("Advanced").getColumns()){
        // The public API exposes semantic complex info, not its raw type ID.
        String complex=c.getComplexInfo()==null?"NONE":"PRESENT";
        String expression=property(c,"Expression");if(expression.equals("UNKNOWN"))expression="NONE";
        System.out.printf("%s\t%s\t%s\tFALSE\t%s\t%s\t%s\t%s\t%s\t%s%n",c.getName(),flag(c.isCalculated()),complex,flag(c.isAutoNumber()),flag(c.isHyperlink()),property(c,"AppendOnly"),property(c,"TextFormat"),property(c,"ResultType"),expression);
      }
    }
  }
}
