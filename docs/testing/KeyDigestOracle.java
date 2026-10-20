import com.healthmarketscience.jackcess.impl.IndexData;
import java.util.HexFormat;

// Original deterministic inputs; calls the reviewed entry-framing reference.
public class KeyDigestOracle {
  public static void main(String[] args) throws Exception {
    var truncate = IndexData.class.getDeclaredMethod("truncateEntryBytes", byte[].class, int.class);
    truncate.setAccessible(true);
    for(int length : new int[]{509, 510, 511, 512, 513, 1024, 2048, 4096}) {
      for(int seed : new int[]{0, 17, 255}) {
        byte[] raw = new byte[length];
        for(int i = 0; i < length; i++) raw[i] = (byte)(i * 37 + seed);
        byte[] key = (byte[])truncate.invoke(null, raw, 510);
        System.out.println(length + "\t" + seed + "\t" + HexFormat.of().formatHex(key, key.length - 2, key.length));
      }
    }
  }
}
