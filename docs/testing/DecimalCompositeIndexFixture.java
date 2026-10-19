import com.healthmarketscience.jackcess.*;
import java.math.*;
import java.nio.file.*;
import java.time.LocalDateTime;
import java.util.TimeZone;

/** Original inert wide-tuple recipe; coefficients never pass through floats. */
public class DecimalCompositeIndexFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action) throws Exception {
    try{action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static ColumnBuilder decimal(String name,int scale) {
    return new ColumnBuilder(name,DataType.NUMERIC).setPrecision(28).setScale(scale);
  }
  static BigDecimal number(BigInteger coefficient,int scale) {return new BigDecimal(coefficient,scale);}
  public static void main(String[] args) throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    BigInteger maximum=BigInteger.TEN.pow(28).subtract(BigInteger.ONE);
    BigInteger[] coefficients={BigInteger.ZERO,BigInteger.ONE,BigInteger.ONE.negate(),
      new BigInteger("9007199254740993"),new BigInteger("9007199254740994"),
      new BigInteger("9223372036854775808"),new BigInteger("-9223372036854775809"),
      new BigInteger("311917102708983781990730508"),maximum,maximum.negate()};
    String zero="{00000000-0000-0000-0000-000000000000}",high="{FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF}";
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      if(((com.healthmarketscience.jackcess.impl.DatabaseImpl)db).getFormat().LEGACY_NUMERIC_INDEXES)
        throw new AssertionError("Expected corrected numeric index encoding");
      String[] keys={"A","G","Flag","B","Amount","N","Tiny","Narrow","S","Moment"};
      Table mixed=new TableBuilder("DecimalMixed").addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addColumn(decimal("A",4)).addColumn(decimal("B",28)).addColumn(new ColumnBuilder("G",DataType.GUID))
        .addColumn(new ColumnBuilder("Amount",DataType.MONEY)).addColumn(new ColumnBuilder("Flag",DataType.BOOLEAN))
        .addColumn(new ColumnBuilder("N",DataType.BIG_INT)).addColumn(new ColumnBuilder("Tiny",DataType.BYTE))
        .addColumn(new ColumnBuilder("Narrow",DataType.INT)).addColumn(new ColumnBuilder("S",DataType.FLOAT))
        .addColumn(new ColumnBuilder("Moment",DataType.SHORT_DATE_TIME))
        .addIndex(new IndexBuilder("MixedAsc").addColumns(keys))
        .addIndex(new IndexBuilder("MixedDirections").addColumns(false,"G","B").addColumns("Flag","N","A")
          .addColumns(false,"Moment","Amount").addColumns("S","Tiny").addColumns(false,"Narrow")).toTable(db);
      for(int i=0;i<1000;i++)mixed.addRow(i,i%7==0?null:number(coefficients[i%coefficients.length],4),
        i%11==0?null:number(coefficients[(i*3)%coefficients.length],28),
        i%13==0?null:String.format("{%08X-89AB-CDEF-0123-%012X}",i%23,i*17L),
        i%17==0?null:BigDecimal.valueOf(9007199254740993L-i*31L,4),i%2==0,
        i%19==0?null:Long.MAX_VALUE-i*43L,i%23==0?null:(byte)(i%256),
        i%29==0?null:(short)(i%101-50),i%31==0?null:(float)(i%7-3)/4,
        i%37==0?null:LocalDateTime.of(2000,1,i%28+1,12,0));
      int[] scales={0,1,2,4,8,12,16,20,24,28};
      TableBuilder ten=new TableBuilder("DecimalTen").addColumn(new ColumnBuilder("ID",DataType.LONG));
      IndexBuilder asc=new IndexBuilder("TenAsc"),desc=new IndexBuilder("TenDesc"),directions=new IndexBuilder("TenMixed");
      for(int i=0;i<10;i++) {
        ten.addColumn(decimal("K"+i,scales[i]));asc.addColumns("K"+i);desc.addColumns(false,"K"+i);directions.addColumns(i%2==0,"K"+i);
      }
      Table tenTable=ten.addIndex(asc).addIndex(desc).addIndex(directions).toTable(db);
      for(int i=0;i<500;i++) {
        Object[] row=new Object[11];row[0]=i;
        for(int j=0;j<10;j++)row[j+1]=(i+j)%17==0?null:number(coefficients[(i+j*3)%coefficients.length],scales[j]);
        tenTable.addRow(row);
      }
      Table unique=new TableBuilder("DecimalTupleUnique").addColumn(decimal("A",4))
        .addColumn(new ColumnBuilder("G",DataType.GUID)).addColumn(decimal("B",28))
        .addIndex(new IndexBuilder("UniqueTuple").addColumns("A","G","B").setUnique()).toTable(db);
      BigDecimal z4=number(BigInteger.ZERO,4),z28=number(BigInteger.ZERO,28),one=number(BigInteger.ONE,28);
      unique.addRow(null,null,null);unique.addRow(null,null,null);unique.addRow(z4,null,null);
      unique.addRow(null,zero,null);unique.addRow(null,null,z28);
      unique.addRow(number(coefficients[3],4),zero,one);unique.addRow(number(coefficients[4],4),zero,one);
      unique.addRow(number(maximum.negate(),4),high,one.negate());unique.addRow(z4,zero,z28);
      rejects(()->unique.addRow(z4,null,null));rejects(()->unique.addRow(null,zero,null));
      rejects(()->unique.addRow(number(coefficients[3],4),zero,one));
      Table primary=new TableBuilder("DecimalTuplePrimary").addColumn(decimal("A",4))
        .addColumn(new ColumnBuilder("G",DataType.GUID)).addColumn(decimal("B",28))
        .addIndex(new IndexBuilder("PrimaryTuple").addColumns("A","G","B").setPrimaryKey()).toTable(db);
      primary.addRow(z4,zero,z28);primary.addRow(number(maximum,4),high,one);
      primary.addRow(number(maximum.negate(),4),zero,one.negate());
      rejects(()->primary.addRow(null,zero,z28));rejects(()->primary.addRow(z4,null,z28));
      rejects(()->primary.addRow(z4,zero,null));rejects(()->primary.addRow(z4,zero,z28));
      new TableBuilder("DecimalTupleEmpty").addColumn(decimal("A",4)).addColumn(new ColumnBuilder("ID",DataType.LONG))
        .addIndex(new IndexBuilder("EmptyTuple").addColumns("A","ID")).toTable(db);
      new TableBuilder("DecimalTextTuple").addColumn(decimal("A",4)).addColumn(new ColumnBuilder("Label",DataType.TEXT).setLengthInUnits(10))
        .addIndex(new IndexBuilder("TextTuple").addColumns("A","Label")).toTable(db);
    }
    System.out.println("Exact Decimal mixed/ten-field tuples, unique/primary and NULL assertions passed.");
  }
}
