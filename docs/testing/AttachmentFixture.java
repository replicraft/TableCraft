import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.time.LocalDateTime;

/** Original inert attachment corpus; fresh generated files and read-only oracle. */
public class AttachmentFixture {
  public static void main(String[] args) throws Exception {
    if(args.length!=2) throw new IllegalArgumentException("Expected fresh path and V2007/V2016/V2019, or --inspect path");
    if(args[0].equals("--metadata")||args[0].equals("--query-metadata")){metadata(Path.of(args[1]),args[0].equals("--query-metadata"));return;}
    if(args[0].equals("--inspect")||args[0].equals("--encodings")){inspect(Path.of(args[1]),args[0].equals("--encodings"));return;}
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()){
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table table=new TableBuilder("Collections").addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true))
        .addColumn(new ColumnBuilder("Files").setAttachment()).toTable(db);
      Object[] row=table.addRow(Column.AUTO_NUMBER,Column.AUTO_NUMBER);
      var key=(ComplexValueForeignKey)table.getColumn("Files").getRowValue(row);
      var time=LocalDateTime.of(2024,2,29,12,0);
      byte[] binary=new byte[256];for(int i=0;i<binary.length;i++)binary[i]=(byte)i;
      byte[] maximum=new byte[65536];java.util.Arrays.fill(maximum,(byte)42);
      key.addAttachment("","empty.txt","txt",new byte[0],time,0);
      key.addAttachment("","binary.bin","bin",binary,time,0);
      key.addAttachment("","unicode.txt","txt","é🦀".getBytes(StandardCharsets.UTF_8),time,0);
      key.addAttachment("","signature.png","png",new byte[]{(byte)137,80,78,71,13,10,26,10},time,0);
      key.addAttachment("","signature.zip","zip",new byte[]{80,75,3,4},time,0);
      key.addAttachment("","maximum.bin","bin",maximum,time,0);
      table.addRow(Column.AUTO_NUMBER,Column.AUTO_NUMBER);
    }
  }
  static String text(String value) {
    return value==null?"N":"T"+java.util.HexFormat.of().formatHex(value.getBytes(StandardCharsets.UTF_8));
  }
  static void metadata(Path path,boolean parents)throws Exception {
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(java.util.TimeZone.getTimeZone("UTC"));
      Table table=db.getTable("Collections");
      var info=(com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl<?>)table.getColumn("Files").getComplexInfo();
      var timestamp=(com.healthmarketscience.jackcess.impl.ColumnImpl)info.getComplexValueForeignKeyColumn().getTable().getColumn("FileTimeStamp");
      int parent=0;
      for(Row row:table) {
        if(parents) {
          if(!(row.get("ID") instanceof Integer id))throw new IllegalArgumentException("Expected original Int32 parent identity");
          System.out.printf("P\t%d\tI32:%d%n",parent,id);
        }
        var key=(ComplexValueForeignKey)row.get("Files");
        for(Attachment attachment:key.getAttachments()) {
          Object time=attachment.getFileTimeStampObject();
          String storedTime="N";
          if(time!=null) {
            var bytes=timestamp.write(time,0,java.nio.ByteOrder.LITTLE_ENDIAN).duplicate();
            if(bytes.remaining()!=8)throw new IllegalArgumentException("Expected ordinary DATE serialization");
            storedTime=String.format("Y64:%016x",bytes.order(java.nio.ByteOrder.LITTLE_ENDIAN).getLong());
          }
          byte[] encoded=attachment.getEncodedFileData();
          System.out.printf("%d\t%d\t%s\t%s\t%s\t%s\t%s\t%s%n",parent,attachment.getId().get(),
            text(attachment.getFileUrl()),text(attachment.getFileName()),text(attachment.getFileType()),
            encoded==null?"N":"X"+java.util.HexFormat.of().formatHex(encoded),storedTime,
            attachment.getFileFlags()==null?"N":"I32:"+attachment.getFileFlags());
        }
        parent++;
      }
    }
  }
  static void inspect(Path path,boolean encodings)throws Exception{
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()){
      int parent=0;for(Row row:db.getTable("Collections")){
        var key=(ComplexValueForeignKey)row.get("Files");
        for(Attachment attachment:key.getAttachments()){
          if(encodings){
            int codec=java.nio.ByteBuffer.wrap(attachment.getEncodedFileData()).order(java.nio.ByteOrder.LITTLE_ENDIAN).getInt();
            System.out.printf("%d\t%d\t%d%n",parent,attachment.getId().get(),codec);continue;
          }
          System.out.printf("%d\t%d\t%s\t%s%n",parent,attachment.getId().get(),
            java.util.HexFormat.of().formatHex(attachment.getFileType().getBytes(StandardCharsets.UTF_8)),
            java.util.HexFormat.of().formatHex(attachment.getFileData()));
        }
        parent++;
      }
    }
  }
}
