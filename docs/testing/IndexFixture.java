import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class IndexFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2) throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019");
    Path path=Path.of(args[0]);if(java.nio.file.Files.exists(path)) throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]) {case "V2000" -> Database.FileFormat.V2000;case "V2007" -> Database.FileFormat.V2007;
      case "V2016" -> Database.FileFormat.V2016;
      case "V2019" -> Database.FileFormat.V2019;default -> throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      Table table=new TableBuilder("Indexed")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Category",DataType.LONG))
        .addColumn(new ColumnBuilder("Label",DataType.TEXT).setLengthInUnits(20))
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("CategoryDesc").addColumns(false,"Category"))
        .addIndex(new IndexBuilder("Composite").addColumns("Category","ID"))
        .addIndex(new IndexBuilder("Label").addColumns("Label"))
        .addIndex(new IndexBuilder("SkipNull").addColumns("Category").setIgnoreNulls())
        .toTable(db);
      for(int i=0;i<1000;i++) table.addRow((i*37)%1000-500,(i%11==0)?null:i%9-4,"row"+i);
      new TableBuilder("Empty").addColumn(new ColumnBuilder("ID",DataType.LONG)).addIndex(new IndexBuilder("ID").addColumns("ID")).toTable(db);
    }
  }
}
