import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original inert Byte/Int16 native index recipe, fresh outputs only. */
public class NarrowIndexFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      for(boolean small:new boolean[]{false,true}) {
        String label=small?"Short":"Byte";DataType type=small?DataType.INT:DataType.BYTE;
        Table table=new TableBuilder(label+"Indexed")
          .addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("PrimaryKey").addColumns("ID").setPrimaryKey())
          .addIndex(new IndexBuilder("KeyAsc").addColumns("Value"))
          .addIndex(new IndexBuilder("KeyDesc").addColumns(false,"Value")).toTable(db);
        int[] shortValues={Short.MIN_VALUE,Short.MIN_VALUE+1,-129,-128,-1,0,1,127,128,Short.MAX_VALUE-1,Short.MAX_VALUE};
        for(int i=0;i<700;i++) {
          Object value=i%11==0?null:(small?shortValues[(i*7)%shortValues.length]:(i*37)%256);
          table.addRow(i,value);
        }
        Table unique=new TableBuilder(label+"Unique").addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("UniqueKey").addColumns("Value").setUnique()).toTable(db);
        if(small) {for(int value:new int[]{Short.MIN_VALUE,Short.MIN_VALUE+1,-1,0,1,Short.MAX_VALUE-1,Short.MAX_VALUE})unique.addRow(value);}
        else {for(int value=0;value<=255;value++)unique.addRow(value);}
        unique.addRow((Object)null);unique.addRow((Object)null);
        try {unique.addRow(0);throw new AssertionError("Duplicate accepted");}catch(ConstraintViolationException expected){}
        new TableBuilder(label+"Empty").addColumn(new ColumnBuilder("ID",DataType.LONG)).addColumn(new ColumnBuilder("Value",type))
          .addIndex(new IndexBuilder("EmptyKey").addColumns("Value")).toTable(db);
      }
    }
  }
}
