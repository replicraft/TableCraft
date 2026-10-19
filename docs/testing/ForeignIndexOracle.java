import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.impl.IndexImpl;
import java.nio.file.Path;

/** Original read-only reciprocal-index comparison; never evaluates application code. */
public class ForeignIndexOracle {
  public static void main(String[] args) throws Exception {
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table=db.getTable(args[1]);
      for(Index raw:table.getIndexes()) {
        IndexImpl index=(IndexImpl)raw;
        if(!index.isForeignKey())continue;
        var ref=index.getReference();
        var other=index.getReferencedIndex();
        if(ref==null||other==null)throw new IllegalStateException("Missing foreign reference");
        int deletes=ref.isCascadeDeletes()?1:ref.isCascadeNullOnDelete()?2:0;
        System.out.println(table.getName()+"\t"+index.getName()+"\t"+index.getIndexNumber()+"\t"+ref.getTableType()+"\t"+ref.getOtherTablePageNumber()+"\t"+ref.getOtherIndexNumber()+"\t"+(ref.isCascadeUpdates()?1:0)+"\t"+deletes+"\t"+other.getName());
      }
    }
  }
}
