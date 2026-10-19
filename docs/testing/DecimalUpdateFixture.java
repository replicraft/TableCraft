import com.healthmarketscience.jackcess.*;
import java.math.*;
import java.nio.file.*;
import java.util.*;

/** Original inert Decimal variable-slot/cascade recipe; fresh outputs only. */
public class DecimalUpdateFixture {
  static final String G="{00112233-4455-6677-8899-AABBCCDDEEFF}";
  static BigDecimal d(Long n,int scale){return n==null?null:new BigDecimal(BigInteger.valueOf(n),scale);}
  static Table table(Database db,String name,boolean unique,boolean indexed,int precision,int scale)throws Exception{
    var t=new TableBuilder(name).addColumn(new ColumnBuilder("Id",DataType.LONG))
      .addColumn(new ColumnBuilder("Notes",DataType.MEMO))
      .addColumn(new ColumnBuilder("Amount",DataType.NUMERIC).setPrecision(precision).setScale(scale))
      .addColumn(new ColumnBuilder("Payload",DataType.OLE))
      .addColumn(new ColumnBuilder("Detail",DataType.NUMERIC).setPrecision(28).setScale(28))
      .addColumn(new ColumnBuilder("Band",DataType.BIG_INT)).addColumn(new ColumnBuilder("G",DataType.GUID))
      .addIndex(new IndexBuilder("Primary").addColumns("Id").setPrimaryKey());
    if(indexed){
      IndexBuilder a=new IndexBuilder("AmountAsc").addColumns("Amount"), b=new IndexBuilder("AmountDesc").addColumns(false,"Amount");
      var c=new IndexBuilder("Tuple").addColumns("Amount").addColumns(false,"Detail").addColumns("Band","G");
      if(unique){a.setUnique();b.setUnique();c.setUnique();}
      t.addIndex(a).addIndex(b).addIndex(c);
    }
    return t.toTable(db);
  }
  static void row(Table t,int id,Long amount,Long detail,long band,String guid,int scale)throws Exception{
    byte[] payload=new byte[id==0?12288:16];Arrays.fill(payload,(byte)id);
    t.addRow(id,id==0?"Unicode é🦀 ".repeat(700):"row "+id,d(amount,scale),payload,d(detail,28),band,guid);
  }
  public static void main(String[] args)throws Exception{
    if(args.length!=2||Files.exists(Path.of(args[0])))throw new IllegalArgumentException("Fresh path and format required");
    var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Format");};
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setFileFormat(format).create()){
      db.setEvaluateExpressions(false);
      for(String prefix:new String[]{"","Tuple"}){
        Table p=table(db,prefix+"Parents",true,true,28,4),c=table(db,prefix+"Children",false,true,28,4);
        for(int i=0;i<4;i++){Long n=i==3?null:(long)i+1;row(p,i,n,n,i,G,4);row(c,i,n,n,i,G,4);}
        row(c,4,1L,1L,0,G,4);
        var relation=new RelationshipBuilder(p,c).addColumns("Amount","Amount").setReferentialIntegrity().setCascadeUpdates().setName(prefix+"Link");
        if(!prefix.isEmpty())relation.addColumns("Detail","Detail").addColumns("Band","Band").addColumns("G","G");
        relation.toRelationship(db);
      }
      Table plain=table(db,"Plain",false,false,4,2);
      row(plain,0,1L,-1L,0,G,2);row(plain,1,null,null,1,G,2);
      Table tree=table(db,"Tree",false,true,28,4);
      for(int i=0;i<1000;i++)row(tree,i,i%11==0?null:(long)(i%31)-15,(long)i, i,G,4);
    }
  }
}
