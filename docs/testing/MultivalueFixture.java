import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.math.BigDecimal;
import java.nio.file.*;

/** Original inert scalar collection recipe, refusing existing paths. */
public class MultivalueFixture {
  static final String[] NAMES={"Bytes","Shorts","Ints","Singles","Doubles","Guids","Decimals","Texts"};
  static final DataType[] TYPES={DataType.BYTE,DataType.INT,DataType.LONG,DataType.FLOAT,
    DataType.DOUBLE,DataType.GUID,DataType.NUMERIC,DataType.TEXT};
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      TableBuilder builder=new TableBuilder("Multivalues").addColumn(new ColumnBuilder("ID",DataType.LONG).setAutoNumber(true));
      for(int i=0;i<NAMES.length;i++)builder.addColumn(new ColumnBuilder(NAMES[i]).setMultiValue(TYPES[i]).setValueList("0","1"));
      Table table=builder.toTable(db);
      Object[][] samples={{0,1,255},{Short.MIN_VALUE,0,Short.MAX_VALUE},{Integer.MIN_VALUE,0,Integer.MAX_VALUE},
        {-0.0f,Float.MIN_VALUE,Float.POSITIVE_INFINITY,Float.intBitsToFloat(0x7fc01234)},
        {-0.0,Double.MIN_VALUE,Double.NEGATIVE_INFINITY,Double.longBitsToDouble(0x7ff8000000001234L)},
        {"{00000000-0000-0000-0000-000000000000}","{00000000-0000-0000-0000-000000000001}","{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}"},
        {new BigDecimal("-999999999999999999"),new BigDecimal("0"),new BigDecimal("9007199254740992"),new BigDecimal("9007199254740993")},
        {"","Red","é🦀","a\tb\nc","Red"}};
      for(int ordinal=0;ordinal<4;ordinal++) {
        Object[] cells=new Object[NAMES.length+1];java.util.Arrays.fill(cells,Column.AUTO_NUMBER);
        Object[] row=table.addRow(cells);
        for(int i=0;i<NAMES.length;i++) {
          var key=(ComplexValueForeignKey)table.getColumn(NAMES[i]).getRowValue(row);
          if(ordinal==0)for(Object value:samples[i])key.addMultiValue(value);
          if(ordinal==1)key.addMultiValue(null);
          if(ordinal==3&&NAMES[i].equals("Ints"))for(int value=0;value<100;value++)key.addMultiValue(value);
        }
      }
      Table overflow=new TableBuilder("Overflow").addColumn(new ColumnBuilder("Items").setMultiValue(DataType.LONG).setValueList("0","1")).toTable(db);
      Object[] row=overflow.addRow(Column.AUTO_NUMBER);
      var key=(ComplexValueForeignKey)overflow.getColumn("Items").getRowValue(row);
      // This independent implementation permits 101; TableCraft's typed view
      // deliberately applies its documented 100-item policy. Raw links still read.
      for(int value=0;value<101;value++)key.addMultiValue(value);
    }
    System.out.println("Original typed multivalue inputs created.");
  }
}
