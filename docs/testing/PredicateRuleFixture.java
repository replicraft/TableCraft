import com.healthmarketscience.jackcess.*;
import java.nio.file.*;

// Original repository-licensed pure numeric rule fixture; fresh output only.
public class PredicateRuleFixture {
 interface Attempt{void run()throws Exception;}
 static void rejects(Attempt f)throws Exception{try{f.run();}catch(InvalidValueException e){return;}throw new IllegalStateException("Expected validation rejection");}
 public static void main(String[] args)throws Exception{
  if(args.length!=2)throw new IllegalArgumentException("Expected fresh path and V2016/V2019, or --inspect path");
  if(args[0].equals("--inspect")){inspect(Path.of(args[1]));return;}
  Path path=Path.of(args[0]);if(Files.exists(path))throw new IllegalArgumentException("Output must not exist");
  var format=switch(args[1]){case "V2016"->Database.FileFormat.V2016;case "V2019"->Database.FileFormat.V2019;default->throw new IllegalArgumentException("Unsupported format");};
  try(Database db=new DatabaseBuilder(path).setFileFormat(format).create()){
   String[] names={"Ranges","Reversed","Allowed","Excluded"};String[] rules={"Between 0 And 100","Between 100 And 0","In (1,3,5)","Not In (1,3,5)"};
   int[][] values={{0,7,100},{0,7,100},{1,3,5},{0,2,4}};int[] invalid={101,-1,2,3};
   for(int i=0;i<names.length;i++){
    Table table=new TableBuilder(names[i]).addColumn(new ColumnBuilder("Score",DataType.LONG).putProperty("Required",true).putProperty("ValidationRule",rules[i])).toTable(db);
    for(int value:values[i])table.addRow(value);
    final int bad=invalid[i];rejects(()->table.addRow(bad));
   }
   new TableBuilder("EmptyText").addColumn(new ColumnBuilder("Label",DataType.TEXT).putProperty("ValidationRule","In ('A','B')")).toTable(db);
   Table unknown=new TableBuilder("NullMembership").addColumn(new ColumnBuilder("Score",DataType.LONG)).toTable(db);unknown.addRow(2);
   // Preserve an existing unknown result, without asserting normal writes accept it.
   unknown.getColumn("Score").getProperties().put("ValidationRule","In (1,Null)");unknown.getColumn("Score").getProperties().save();
  }
  inspect(path);
 }
 static void inspect(Path path)throws Exception{
  try(Database db=new DatabaseBuilder(path).setReadOnly(true).open()){
   for(String name:new String[]{"Ranges","Reversed","Allowed","Excluded","EmptyText","NullMembership"})System.out.printf("%s\t%d%n",name,db.getTable(name).getRowCount());
  }
 }
}
