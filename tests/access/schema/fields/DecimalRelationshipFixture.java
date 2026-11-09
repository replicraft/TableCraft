import com.healthmarketscience.jackcess.*;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.file.*;

/** Original inert exact Decimal relationship recipe; no active content. */
public class DecimalRelationshipFixture {
  interface Attempt {void run() throws Exception;}
  static void rejects(Attempt action)throws Exception {
    try {action.run();}catch(ConstraintViolationException expected){return;}
    throw new AssertionError("Expected constraint rejection");
  }
  static BigDecimal decimal(String coefficient,int scale) {
    return new BigDecimal(new BigInteger(coefficient),scale);
  }
  static ColumnBuilder column(String name,int scale) {
    return new ColumnBuilder(name,DataType.NUMERIC).setPrecision(28).setScale(scale);
  }
  static Table table(Database db,String name,boolean unique,int scale,boolean mixed)throws Exception {
    TableBuilder builder=new TableBuilder(name).addColumn(column("Amount",scale));
    if(mixed)builder.addColumn(new ColumnBuilder("Band",DataType.LONG)).addColumn(column("Detail",28));
    if(unique)builder.addIndex(new IndexBuilder("UniqueKey")
      .addColumns(mixed?new String[]{"Amount","Band","Detail"}:new String[]{"Amount"}).setUnique());
    return builder.toTable(db);
  }
  static void relation(Database db,Table parent,Table child,String name,boolean mixed)throws Exception {
    RelationshipBuilder builder=new RelationshipBuilder(parent,child).addColumns("Amount","Amount");
    if(mixed)builder.addColumns("Band","Band").addColumns("Detail","Detail");
    builder.setReferentialIntegrity().setName(name).toRelationship(db);
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2000/V2007/V2016/V2019");
    Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
    Database.FileFormat format=switch(args[1]){case "V2000"->Database.FileFormat.V2000;case "V2007"->Database.FileFormat.V2007;case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
    try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()) {
      String[] coefficients={"-9999999999999999999999999999","-9223372036854775809","-9223372036854775808",
        "-9007199254740993","-1","0","1","9007199254740992","9007199254740993",
        "9223372036854775807","9223372036854775808","9999999999999999999999999999"};
      for(int scale:new int[]{0,4,28}) {
        String prefix="Decimal"+scale;
        Table parent=table(db,prefix+"Parents",true,scale,false),child=table(db,prefix+"Children",false,scale,false),
          unique=table(db,prefix+"UniqueChildren",true,scale,false);
        relation(db,parent,child,prefix+"Keys",false);relation(db,parent,unique,prefix+"UniqueKeys",false);
        for(String value:coefficients){parent.addRow(decimal(value,scale));child.addRow(decimal(value,scale));}
        for(int i=0;i<2;i++){parent.addRow((Object)null);child.addRow((Object)null);}
        unique.addRow(decimal(coefficients[0],scale));unique.addRow(decimal(coefficients[1],scale));
        unique.addRow((Object)null);unique.addRow((Object)null);
        rejects(()->child.addRow(decimal("2",scale)));
        rejects(()->unique.addRow(decimal(coefficients[0],scale)));
      }
      Table parent=table(db,"DecimalMixedParents",true,4,true),child=table(db,"DecimalMixedChildren",false,4,true),
        unique=table(db,"DecimalMixedUniqueChildren",true,4,true);
      relation(db,parent,child,"DecimalMixedKeys",true);relation(db,parent,unique,"DecimalMixedUniqueKeys",true);
      Object[][] tuples={{decimal(coefficients[0],4),Integer.MIN_VALUE,decimal(coefficients[11],28)},
        {decimal(coefficients[11],4),Integer.MAX_VALUE,decimal(coefficients[0],28)},
        {decimal("9007199254740992",4),7,decimal("1",28)},
        {decimal("9007199254740993",4),8,decimal("1",28)},
        {null,Integer.MAX_VALUE,decimal("0",28)},
        {decimal(coefficients[11],4),null,null},{null,null,null},{null,null,null}};
      for(Object[] tuple:tuples){parent.addRow(tuple);child.addRow(tuple);}
      unique.addRow(tuples[4]);unique.addRow(tuples[5]);unique.addRow(null,null,null);unique.addRow(null,null,null);
      rejects(()->child.addRow(decimal(coefficients[0],4),Integer.MAX_VALUE,decimal(coefficients[11],28)));
      rejects(()->child.addRow(decimal("9007199254740993",4),7,decimal("1",28)));
      rejects(()->child.addRow(null,Integer.MIN_VALUE,decimal("0",28)));
      rejects(()->child.addRow(decimal(coefficients[11],4),null,decimal("0",28)));
      rejects(()->unique.addRow(tuples[4]));
    }
    System.out.println("All exact Decimal relationship assertions passed.");
  }
}
