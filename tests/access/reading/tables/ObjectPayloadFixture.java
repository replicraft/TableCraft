import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert opaque-byte storage recipe, not valid VBA/application bodies. */
public class ObjectPayloadFixture {
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      for(String name:new String[]{"OpaqueObject","NullObject","EmptyObject","LargeObject"})new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG)).toTable(db);
      byte[] body=new byte[2304],module=new byte[12288];
      for(int i=0;i<body.length;i++)body[i]=(byte)i;
      for(int i=0;i<module.length;i++)module[i]=(byte)(i*37+11);
      Table catalog=db.getSystemTable("MSysObjects");
      for(String field:new String[]{"Lv","LvModule","LvExtra"})catalog.getColumn(field);
      for(Row row:catalog) {
        switch(row.getString("Name")) {
          case "OpaqueObject"->{row.put("Lv",body);row.put("LvModule",module);row.put("LvExtra",new byte[]{0,1,0,(byte)255});catalog.updateRow(row);}
          case "EmptyObject"->{row.put("Lv",new byte[0]);row.put("LvModule",new byte[0]);row.put("LvExtra",new byte[0]);catalog.updateRow(row);}
          case "LargeObject"->{row.put("Lv",new byte[]{1,2,3});row.put("LvModule",new byte[65537]);catalog.updateRow(row);}
          default->{}
        }
      }
    }
  }
}
