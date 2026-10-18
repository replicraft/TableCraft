import com.healthmarketscience.jackcess.*;
import java.nio.file.Path;
public class BinaryFixture {
  public static void main(String[] args) throws Exception {
    if(java.nio.file.Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Output must not exist");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.V2007).create()) {
      Table table=new TableBuilder("BinarySamples")
        .addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Payload",DataType.OLE))
        .toTable(db);
      table.addRow(1,new byte[]{0,(byte)255,0,(byte)216});
      byte[] large=new byte[12000];for(int i=0;i<large.length;i++) large[i]=(byte)(i%251);
      table.addRow(2,large);
      table.addRow(3,new byte[0]);
      table.addRow(4,null);
    }
  }
}
