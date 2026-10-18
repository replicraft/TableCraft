import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.nio.file.*;
import java.time.LocalDateTime;

// Original repository-licensed fixture; creates trusted data on a fresh file only.
public class ComplexRowsFixture {
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019, or --tables/--links path");
    if(args[0].equals("--tables")){tables(Path.of(args[1]));return;}
    if(args[0].equals("--links")){links(Path.of(args[1]));return;}
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()){
      db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
      Table table=new TableBuilder("Collections")
        .addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true))
        .addColumn(new ColumnBuilder("Choices").setMultiValue(DataType.TEXT).setValueList("Red","Blue","é🦀"))
        .addColumn(new ColumnBuilder("Files").setAttachment())
        .addColumn(new ColumnBuilder("Notes",DataType.MEMO).setAppendOnly(true)).toTable(db);
      LocalDateTime time=LocalDateTime.of(2024,2,29,12,0);
      for(int i=0;i<3;i++){
        Object[] row=table.addRow(Column.AUTO_NUMBER,Column.AUTO_NUMBER,Column.AUTO_NUMBER,null);
        var choices=(ComplexValueForeignKey)table.getColumn("Choices").getRowValue(row);
        var files=(ComplexValueForeignKey)table.getColumn("Files").getRowValue(row);
        if(i==0){choices.addMultiValue("Red");choices.addMultiValue("Blue");files.addAttachment("","empty.txt","txt",new byte[0],time,0);files.addAttachment("","bytes.bin","bin",new byte[]{0,1,2,(byte)255},time,0);}
        if(i==1){choices.addMultiValue("é🦀");files.addAttachment("","unicode.txt","txt","é🦀".getBytes(java.nio.charset.StandardCharsets.UTF_8),time,0);}
        for(Column c:table.getColumns())if(c.getComplexInfo()!=null&&c.getComplexInfo().getType()==ComplexDataType.VERSION_HISTORY){
          var key=(ComplexValueForeignKey)c.getRowValue(row);
          if(i==0){key.addVersion("older",time);key.addVersion("newer",time.plusDays(1));}
          if(i==1)key.addVersion("",time);
        }
      }
    }
    tables(path);
  }
  static void tables(Path path)throws Exception {
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()){
      for(Column c:db.getTable("Collections").getColumns())if(c.getComplexInfo()!=null){
        var info=(com.healthmarketscience.jackcess.impl.complex.ComplexColumnInfoImpl<?>)c.getComplexInfo();
        System.out.printf("%s\t%s\t%d%n",c.getName(),info.getComplexValueForeignKeyColumn().getTable().getName(),info.getComplexValueForeignKeyColumn().getTable().getRowCount());
      }
    }
  }
  static void links(Path path)throws Exception {
    try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()){
      Table table=db.getTable("Collections");int ordinal=0;
      for(Row row:table){
        for(Column column:table.getColumns())if(column.getComplexInfo()!=null){
          var key=(ComplexValueForeignKey)row.get(column.getName());
          System.out.printf("%s\t%d\t%d\t%d%n",column.getName(),ordinal,key.get(),key.countValues());
        }
        ordinal++;
      }
    }
  }
}
