import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.*;
import java.nio.file.Path;
public class IndexOracle {
  public static void main(String[] args) throws Exception {
    var root=IndexData.class.getDeclaredMethod("getRootPageNumber");root.setAccessible(true);
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      var names=new java.util.TreeSet<String>();
      if(args.length==1) {names.addAll(db.getTableNames());names.addAll(db.getSystemTableNames());}
      else for(int i=1;i<args.length;i++) names.add(args[i]);
      for(String name:names) {
        Table table=db.getSystemTable(name);if(table==null) table=db.getTable(name);
        for(Index raw:table.getIndexes()) {
          IndexImpl index=(IndexImpl)raw;
          for(var column:index.getColumns()) {
            System.out.println(name+"\t"+index.getName()+"\t"+index.getIndexNumber()+"\t"+index.getIndexData().getIndexDataNumber()+"\t"+root.invoke(index.getIndexData())+"\t"+(index.isForeignKey()?2:index.isPrimaryKey()?1:0)+"\t"+String.format("0x%02x",index.getIndexFlags())+"\t"+column.getColumn().getName()+"\t"+(!column.isAscending()?"TRUE":"FALSE"));
          }
        }
      }
    }
  }
}
