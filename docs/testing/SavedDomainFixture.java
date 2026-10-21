import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

/** Original domain-call metadata; no SQL, UDF, VBA or expression is executed. */
public class SavedDomainFixture extends SavedStatisticsFixture {
  static void output(int id,String expression,String name,int order)throws Exception {
    row(id,(byte)6,expression,null,name,null,order);
  }
  static void scalar(String name,String expression)throws Exception {
    output(definition(name,0,1),expression,"V",1);
  }
  public static void main(String[] args)throws Exception {
    SavedStatisticsFixture.main(args);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false);db.setAllowAutoNumberInsert(true);
      catalog=db.getSystemTable("MSysObjects");queries=db.getSystemTable("MSysQueries");
      Table unicode=new TableBuilder("Commandes été").addColumn(new ColumnBuilder("Montant",DataType.LONG)).toTable(db);
      unicode.addRow(3);unicode.addRow(7);
      int id=definition("DomainAll",0,1);
      String[] calls={"DAvg","DCount","DFirst","DLast","DLookup","DMin","DMax","DSum","DVar","DVarP","DStDev","DStDevP"};
      for(int i=0;i<calls.length;i++)output(id,calls[i]+"('ID','AData')","C"+i,i+1);
      output(id,"DCount('*','AData')","N",calls.length+1);
      id=definition("DomainOmitted",0,1);
      for(int i=0;i<calls.length;i++)output(id,calls[i]+"('ID','AData',)","C"+i,i+1);
      output(id,"DCount('*','AData',)","N",calls.length+1);
      scalar("DomainCriteria","DCount('*','AData',\"ID>DMin('ID','BData')\")");
      scalar("DomainMissingField","DLookup('ID','AData','Missing=1')");
      scalar("DomainUnicode","DSum('Montant','Commandes été')");
      scalar("DomainNamed","DSum('V','StatsDerived')");
      scalar("DomainParamRefuse","DCount('*','StatsParams')");
      scalar("DomainRecursive","DCount('V','DomainRecursive')");
      scalar("DomainUnknown","DCount('*','NoSuchDomain')");
      scalar("DomainLateError","DSum('1/(ID-2)','AData')");
      id=definition("DomainParams",0,1);
      row(id,(byte)2,null,(short)10,"Criteria",null,1);
      output(id,"DSum('ID','AData',[Criteria])","V",1);
      id=definition("DomainGrouped",0,1);from(id);
      output(id,"ID Mod 2","G",1);
      output(id,"DCount('*','AData',IIf(ID Mod 2=0,'ID Mod 2=0','ID Mod 2=1'))","N",2);
      output(id,"Count(*)","Rows",3);
      row(id,(byte)9,"ID Mod 2",null,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      id=definition("DomainDerived",0,1);
      row(id,(byte)5,"SELECT DSum('ID','AData') AS V",null,null,"d",1);
      output(id,"d.V","V",1);
      id=definition("DomainUnion",0x80,9);
      row(id,(byte)5,"SELECT DMin('ID','AData') AS V",null,null,"X7YZ_____1",1);
      row(id,(byte)5,"SELECT DMax('ID','AData') AS V",null,null,"X7YZ_____2",2);
      id=definition("DomainCrosstab",0x10,6);from(id);
      row(id,(byte)6,"ID Mod 2",(short)2,"G",null,1);
      row(id,(byte)6,"Sum(DCount('*','AData'))",(short)2,"Total",null,2);
      row(id,(byte)6,"Sum(DCount('*','AData'))",(short)0,"Measure",null,3);
      row(id,(byte)6,"ID IN (1,2,4)",(short)1,null,null,4);
      row(id,(byte)9,"ID Mod 2",(short)2,null,null,1);row(id,(byte)11,"G",null,"A",null,1);
      db.flush();if(db.getQueries().size()!=38)throw new AssertionError("Query count changed");
    }
    System.out.println("Original domain query definitions created without execution.");
  }
}
