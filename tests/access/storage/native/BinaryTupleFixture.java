import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;
import java.math.BigDecimal;
import java.time.LocalDateTime;

// Original inert inputs. Native Microsoft Access creation/save remains unrun.
public class BinaryTupleFixture {
  static byte[][] values() {
    List<byte[]> result = new ArrayList<>(); result.add(null); result.add(new byte[0]);
    for(int length : new int[]{1,2,7,8,9,15,16,17,254,255}) {
      for(int seed : new int[]{0,128,255}) {
        byte[] value = new byte[length];
        for(int i=0;i<length;i++) value[i]=(byte)(seed==128 ? seed+i*31 : seed);
        result.add(value);
      }
    }
    return result.toArray(byte[][]::new);
  }
  static TableBuilder pair(String name) {
    return new TableBuilder(name).addColumn(new ColumnBuilder("ID", DataType.LONG))
      .addColumn(new ColumnBuilder("Bytes", DataType.BINARY).setLength(255));
  }
  static IndexBuilder index(String name, boolean ascending, String... columns) {
    return new IndexBuilder(name).addColumns(ascending, columns);
  }
  public static void main(String[] args) throws Exception {
    if(args.length!=2 || Files.exists(Path.of(args[0]))) throw new IllegalArgumentException("Fresh path and format required");
    byte[][] values=values();
    try(Database db = new DatabaseBuilder(Path.of(args[0])).setFileFormat(Database.FileFormat.valueOf(args[1])).create()) {
      db.setEvaluateExpressions(false); db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table pairs=pair("Pairs").addIndex(index("BinaryNumber",true,"Bytes","ID"))
        .addIndex(index("NumberBinary",false,"ID","Bytes"))
        .addIndex(new IndexBuilder("MixedDirection").addColumns(false,"Bytes").addColumns(true,"ID")).toTable(db);
      for(int i=0;i<1000;i++) pairs.addRow(i,values[(i*17)%values.length]);
      TableBuilder mixed=new TableBuilder("Mixed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Bytes",DataType.BINARY).setLength(255))
        .addColumn(new ColumnBuilder("Other",DataType.BINARY).setLength(255))
        .addColumn(new ColumnBuilder("Token",DataType.GUID))
        .addColumn(new ColumnBuilder("Amount",DataType.NUMERIC).setPrecision(28).setScale(4))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Byte",DataType.BYTE))
        .addColumn(new ColumnBuilder("Short",DataType.INT))
        .addColumn(new ColumnBuilder("Price",DataType.MONEY))
        .addColumn(new ColumnBuilder("Real",DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Day",DataType.SHORT_DATE_TIME));
      String[] keys={"Bytes","Other","Token","Amount","Flag","Byte","Short","Price","Real","Day"};
      mixed.addIndex(index("TenAsc",true,keys)).addIndex(index("TenDesc",false,keys));
      IndexBuilder alternating=new IndexBuilder("Alternating");
      for(int i=0;i<keys.length;i++) alternating.addColumns(i%2==0,keys[i]);
      Table table=mixed.addIndex(alternating).toTable(db);
      for(int i=0;i<64;i++) {
        boolean nil=i%13==0;
        table.addRow(i, values[i%values.length],values[(i*3+1)%values.length],
          nil?null:String.format("{%08x-1234-5678-abcd-0123456789ab}",i),
          nil?null:new BigDecimal(i-32).movePointLeft(4).setScale(4), i%2==0,
          nil?null:i*3,nil?null:i-32,nil?null:new BigDecimal(i-32).movePointLeft(4).setScale(4),
          nil?null:(i-32)/4.0,nil?null:LocalDateTime.of(2020,1,1,0,0).plusDays(i));
      }
      Table unique=pair("UniquePairs").addIndex(index("UniquePair",true,"Bytes","ID").setUnique()).toTable(db);
      unique.addRow(null,null); unique.addRow(null,null); unique.addRow(1,new byte[0]); unique.addRow(2,new byte[0]); unique.addRow(null,new byte[]{0});
      pair("EmptyPairs").addIndex(index("EmptyPair",false,"Bytes","ID")).toTable(db);
      Table digest=new TableBuilder("DigestUnique").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(new ColumnBuilder("Bytes",DataType.BINARY).setLength(255)).addColumn(new ColumnBuilder("Other",DataType.BINARY).setLength(255))
        .addIndex(index("UniqueDigest",true,"Bytes","Other").setUnique()).toTable(db);
      digest.addRow(1,values[values.length-1],values[values.length-1]);
      pair("IgnoredPairs").addIndex(index("IgnoredPair",true,"Bytes","ID").setIgnoreNulls()).toTable(db).addRow(1,new byte[]{1});
    }
  }
}
