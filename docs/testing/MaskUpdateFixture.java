import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.nio.file.*;
import java.time.LocalDateTime;
import java.util.*;

/** Original inert Boolean/null-mask recipe; only fresh independent fixtures. */
public class MaskUpdateFixture {
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table items=new TableBuilder("Items")
        .addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("B1",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("Tiny",DataType.BYTE))
        .addColumn(new ColumnBuilder("Narrow",DataType.INT))
        .addColumn(new ColumnBuilder("Number",DataType.LONG))
        .addColumn(new ColumnBuilder("Large",DataType.BIG_INT))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY))
        .addColumn(new ColumnBuilder("B7",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("B8",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("S",DataType.FLOAT))
        .addColumn(new ColumnBuilder("D",DataType.DOUBLE))
        .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
        .addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("B13",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("B14",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("MustHave",DataType.LONG).putProperty("Required",true).putProperty("ValidationRule",">=0"))
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO))
        .addColumn(new ColumnBuilder("Payload",DataType.OLE))
        .addIndex(new IndexBuilder("PK_Items").addColumns("Id").setPrimaryKey()).toTable(db);
      for(int id=1;id<=4;id++) {
        boolean missing=id==2||id==3;byte[] payload=new byte[12288];Arrays.fill(payload,(byte)(160+id));
        items.addRow(id,id%2==0,missing?null:(byte)1,missing?null:Short.MIN_VALUE,missing?null:Integer.MIN_VALUE,
          missing?null:Long.MIN_VALUE,missing?null:BigDecimal.valueOf(Long.MIN_VALUE,4),id%2!=0,id%3==0,
          missing?null:Float.intBitsToFloat(0x80000000),missing?null:Double.longBitsToDouble(0x8000000000000000L),
          missing?null:LocalDateTime.of(1899,12,29,6,0),missing?null:"{00112233-4455-6677-8899-AABBCCDDEEFF}",
          true,false,12,"Unicode é🦀 ".repeat(700),payload);
      }
      Table parents=new TableBuilder("Parents").addColumn(new ColumnBuilder("Key",DataType.BIG_INT))
        .addIndex(new IndexBuilder("PK_Parents").addColumns("Key").setPrimaryKey()).toTable(db);
      Table children=new TableBuilder("Children").addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("ForeignKey",DataType.BIG_INT))
        .addIndex(new IndexBuilder("PK_Children").addColumns("Id").setPrimaryKey()).toTable(db);
      parents.addRow(1L);children.addRow(1,null);children.addRow(2,1L);
      new RelationshipBuilder(parents,children).addColumns("Key","ForeignKey")
        .setReferentialIntegrity().setName("LongForeignKey").toRelationship(db);
      Table indexed=new TableBuilder("IndexedBoolean").addColumn(new ColumnBuilder("Id",DataType.LONG))
        .addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addIndex(new IndexBuilder("FlagIndex").addColumns("Flag")).toTable(db);
      indexed.addRow(1,false);indexed.addRow(2,true);
    }
    System.out.println("Original Boolean/nullable scalar, required and relationship fixtures created without application execution.");
  }
}
