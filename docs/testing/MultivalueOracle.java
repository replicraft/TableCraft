import com.healthmarketscience.jackcess.*;
import com.healthmarketscience.jackcess.complex.*;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.HexFormat;

/** Read-only public collection API oracle; lookup/active expressions disabled. */
public class MultivalueOracle {
  static String encode(Object value,DataType type) {
    if(value==null)return "N";
    return switch(type) {
      case BYTE->"U"+Byte.toUnsignedInt((Byte)value);
      case INT->"I16:"+value;
      case LONG->"I32:"+value;
      case BIG_INT->"I64:"+value;
      case FLOAT->String.format("F%08x",Float.floatToRawIntBits((Float)value));
      case DOUBLE->String.format("D%016x",Double.doubleToRawLongBits((Double)value));
      case GUID->"G"+value.toString().replace("{","").replace("}","").replace("-","").toLowerCase(java.util.Locale.ROOT);
      case NUMERIC->"Q"+((BigDecimal)value).unscaledValue()+":"+((BigDecimal)value).scale();
      case TEXT->"T"+HexFormat.of().formatHex(((String)value).getBytes(StandardCharsets.UTF_8));
      default->throw new IllegalArgumentException("Unqualified multivalue carrier");
    };
  }
  public static void main(String[] args)throws Exception {
    if(args.length!=3 && args.length!=4)throw new IllegalArgumentException("Expected path, table, column and optional parent scalar field");
    try(Database db=new DatabaseBuilder(Path.of(args[0])).setReadOnly(true).open()) {
      db.setEvaluateExpressions(false);
      Table table=db.getTable(args[1]); Column column=table.getColumn(args[2]);
      if(column.getComplexInfo()==null||column.getComplexInfo().getType()!=ComplexDataType.MULTI_VALUE)
        throw new IllegalArgumentException("Expected multivalue column");
      DataType type=((com.healthmarketscience.jackcess.impl.complex.MultiValueColumnInfoImpl)column.getComplexInfo()).getValueColumn().getType();
      int ordinal=0;
      for(Row row:table) {
        var key=(ComplexValueForeignKey)row.get(column.getName());
        StringBuilder output=new StringBuilder(table.getName()).append('\t').append(ordinal++).append('\t').append(key==null?"N":key.get());
        if(args.length==4)output.append('\t').append(encode(row.get(args[3]),table.getColumn(args[3]).getType()));
        if(key!=null)for(SingleValue value:key.getMultiValues())output.append('\t').append(encode(value.get(),type));
        System.out.println(output);
      }
    }
  }
}
