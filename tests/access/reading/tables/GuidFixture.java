import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class GuidFixture {
  public static void main(String[] args) throws Exception {
    if(java.nio.file.Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Output must not exist");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.V2007).create()) {
      Table table=new TableBuilder("GuidSamples")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("Bytes",DataType.BINARY).setLength(16))
        .toTable(db);
      table.addRow(1,"{6b29fc40-ca47-1067-b31d-00dd010662da}",new byte[]{0,(byte)255});
      table.addRow(2,"{00000000-0000-0000-0000-000000000000}",new byte[0]);
      table.addRow(3,null,null);
    }
  }
}
