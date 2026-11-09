import com.healthmarketscience.jackcess.*;
import java.nio.file.*;
import java.util.*;

/** Original already-delimited native name metadata; never evaluates SQL. */
public class SavedIdentifiersFixture extends SavedForestFixture {
  static int definition(String name) throws Exception {
    int id = nextId++;
    Map<String,Object> object = new HashMap<>();
    object.put("Id",id); object.put("ParentId",parent); object.put("Name",name);
    object.put("Type",(short)5); object.put("Flags",0); catalog.addRowFromMap(object);
    row(id,(byte)1,null,(short)1,null,null,1);
    row(id,(byte)3,null,(short)0,null,null,1);
    return id;
  }
  static void select(String name, String table, String alias, String expression,
                     String label, String parameter) throws Exception {
    int id = definition(name);
    row(id,(byte)5,null,null,table,alias,1);
    row(id,(byte)6,expression,null,label,null,1);
    row(id,(byte)11,label,null,"A",null,1);
    if(parameter != null) {
      row(id,(byte)2,null,(short)4,parameter,null,1);
      row(id,(byte)8,"[ID]>="+parameter,null,null,null,1);
    }
  }
  static void joined(String name, boolean aliases) throws Exception {
    int id = definition(name);
    String left = aliases ? "[a]" : "[AData]";
    String right = aliases ? "[b]" : "[BData]";
    row(id,(byte)5,null,null,"[AData]",aliases ? left : null,1);
    row(id,(byte)5,null,null,"[BData]",aliases ? right : null,2);
    row(id,(byte)6,left+".[ID]",null,"[KeyA]",null,1);
    row(id,(byte)6,right+".[ID]",null,"[KeyB]",null,2);
    row(id,(byte)7,left+".[ID]="+right+".[ID]",(short)2,left,right,1);
    row(id,(byte)11,"[KeyA]",null,"A",null,1);
    row(id,(byte)11,"[KeyB]",null,"A",null,2);
  }
  public static void main(String[] args) throws Exception {
    SavedForestFixture.main(args);
    try(Database db = new DatabaseBuilder(Path.of(args[0])).open()) {
      db.setEvaluateExpressions(false); db.setAllowAutoNumberInsert(true);
      catalog = db.getSystemTable("MSysObjects"); queries = db.getSystemTable("MSysQueries");
      select("NamesQuotedTable","[AData]",null,"[AData].[ID]","KeyA",null);
      select("NamesQuotedAlias","AData","[q]","[q].[ID]","KeyA",null);
      select("NamesQuotedColumn","AData",null,"[ID]","[Chosen Key]",null);
      select("NamesQuotedParameter","[AData]",null,"[ID]","KeyA","[Floor]");
      select("NamesQuotedPrompt","AData",null,"[ID]","KeyA","[Enter a minimum]");
      joined("NamesQuotedJoin",false); joined("NamesQuotedJoinAlias",true);
      Table symbols = new TableBuilder("Order # / (items)")
          .addColumn(new ColumnBuilder("Cost (%)",DataType.LONG)).toTable(db);
      for(int value : new int[]{1,2,3}) symbols.addRow(value);
      select("NamesSymbols","[Order # / (items)]","[Select]","[Select].[Cost (%)]","[Value + #]",null);
      select("NamesUnicodeAlias","[AData]","[é]","[é].[ID]","[Chosen Key]",null);
      int id = definition("NamesDerivedAlias");
      row(id,(byte)5,"SELECT ID FROM AData",null,null,"[d]",1);
      row(id,(byte)6,"[d].[ID]",null,"[KeyA]",null,1);
      row(id,(byte)11,"[KeyA]",null,"A",null,1);
      db.flush();
      if(db.getQueries().size()!=24) throw new AssertionError("Query count changed");
    }
    System.out.println("Original stored name inputs created; no query executed.");
  }
}
