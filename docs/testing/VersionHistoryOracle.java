import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import com.healthmarketscience.jackcess.impl.ColumnImpl;
import com.healthmarketscience.jackcess.impl.complex.VersionHistoryColumnInfoImpl;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.*;

/** Read-only stored-order oracle; --api-order separately observes reference sorting. */
public class VersionHistoryOracle {
  static void append(StringBuilder output,Object id,String text,Object time,ColumnImpl date)throws Exception {
    output.append("\tI32:").append(id).append('\t');
    output.append(text==null?"N":"T"+HexFormat.of().formatHex(text.getBytes(StandardCharsets.UTF_8))).append('\t');
    if(time==null)output.append('N');
    else {
      var bytes=date.write(time,0,ByteOrder.LITTLE_ENDIAN).duplicate();
      if(bytes.remaining()!=8)throw new IllegalArgumentException("Expected ordinary DATE serialization");
      output.append(String.format("Y64:%016x",bytes.order(ByteOrder.LITTLE_ENDIAN).getLong()));
    }
  }
  public static void main(String[] args)throws Exception {
    if(args.length<3||args.length>4||(args.length==4&&!args[3].equals("--api-order")))
      throw new IllegalArgumentException("Expected path, table, visible Memo and optional --api-order");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);db.setDateTimeType(DateTimeType.DATE);db.setTimeZone(TimeZone.getTimeZone("UTC"));
      Table table=db.getTable(args[1]);Column hidden=table.getColumn(args[2]).getVersionHistoryColumn();
      if(hidden==null)throw new IllegalArgumentException("Memo has no resolved history");
      var info=(VersionHistoryColumnInfoImpl)hidden.getComplexInfo();
      ColumnImpl date=(ColumnImpl)info.getModifiedDateColumn();
      Column foreign=info.getComplexValueForeignKeyColumn(),identity=info.getPrimaryKeyColumn();
      int parent=0;
      for(Row row:table) {
        var key=(ComplexValueForeignKey)row.get(hidden.getName());
        StringBuilder output=new StringBuilder(table.getName()).append('\t').append(parent++).append('\t').append(key==null?"N":key.get());
        if(key!=null) {
          if(args.length==4)for(Version value:key.getVersions())
            append(output,value.getId().get(),value.getValue(),value.getModifiedDateObject(),date);
          else for(Row value:foreign.getTable())if(Objects.equals(value.get(foreign.getName()),key.get()))
            append(output,value.get(identity.getName()),(String)value.get(info.getValueColumn().getName()),value.get(date.getName()),date);
        }
        System.out.println(output);
      }
    }
  }
}
