import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

// Original inert binary key values and native indexes; fresh files only.
public class BinaryIndexFixture {
  static byte[][] values() {
    List<byte[]> values=new ArrayList<>();values.add(null);values.add(new byte[0]);
    for(int length:new int[]{1,2,7,8,9,15,16,17,254,255}) {
      byte[] zero=new byte[length];values.add(zero);
      byte[] high=new byte[length];Arrays.fill(high,(byte)0xff);values.add(high);
      byte[] mixed=new byte[length];for(int i=0;i<length;i++)mixed[i]=(byte)(i*31+128);values.add(mixed);
    }
    return values.toArray(byte[][]::new);
  }
  static TableBuilder builder(String name) {
    return new TableBuilder(name).addColumn(new ColumnBuilder("ID",DataType.LONG))
      .addColumn(new ColumnBuilder("Bytes",DataType.BINARY).setLength(255));
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2 || Files.exists(Path.of(args[0])))throw new IllegalArgumentException("Expected fresh path and format");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false);
      Table table=builder("BinaryIndexed")
        .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
        .addIndex(new IndexBuilder("BinaryAsc").addColumns("Bytes"))
        .addIndex(new IndexBuilder("BinaryDesc").addColumns(false,"Bytes")).toTable(db);
      byte[][] values=values();
      for(int i=0;i<1000;i++)table.addRow(i,values[(i*17)%values.length]);
      Table unique=builder("BinaryUnique").addIndex(new IndexBuilder("UniqueBinary").addColumns("Bytes").setUnique()).toTable(db);
      for(int i=0;i<values.length;i++)unique.addRow(i,values[i]);
      builder("BinaryEmpty").addIndex(new IndexBuilder("EmptyBinary").addColumns("Bytes")).toTable(db);
      builder("BinaryIgnored").addIndex(new IndexBuilder("IgnoredBinary").addColumns("Bytes").setIgnoreNulls()).toTable(db).addRow(1,new byte[]{1});
      builder("BinaryCompound").addIndex(new IndexBuilder("CompoundBinary").addColumns("Bytes","ID")).toTable(db).addRow(1,new byte[]{1});
    }
  }
}
