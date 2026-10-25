import io.github.spannm.jackcess.Database;
import io.github.spannm.jackcess.DatabaseBuilder;
import io.github.spannm.jackcess.DateTimeType;
import io.github.spannm.jackcess.TableMetaData;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.Base64;
import java.util.List;
import java.util.Properties;
import net.ucanaccess.jdbc.IJackcessOpenerInterface;

/** Original bounded metadata probe; only original offline scalar fixtures are inputs. */
public final class JdbcAggregateMetadataProbe {
    private static final int QUERY_TIMEOUT_SECONDS = 15;
    private static final int MAX_RESULT_ROWS = 16;
    private static Path authorizedDatabase;

    public static final class ReadOnlyOpener implements IJackcessOpenerInterface {
        public Database open(File file, String password) throws IOException {
            if (authorizedDatabase == null || !file.toPath().toRealPath().equals(authorizedDatabase)) {
                throw new IOException("Database target is not authorized by this probe");
            }
            Database database = new DatabaseBuilder().withFile(file).withReadOnly(true).open();
            database.setEvaluateExpressions(false);
            database.setDateTimeType(DateTimeType.LOCAL_DATE_TIME);
            database.setLinkResolver((source, token) -> {
                throw new IOException("Linked databases are not authorized by this probe");
            });
            return database;
        }
    }

    private static String encoded(String value) {
        return Base64.getEncoder().encodeToString(value.getBytes(StandardCharsets.UTF_8));
    }

    private static void query(Connection connection, String label, String sql) {
        System.out.println("QUERY\t" + label + "\t" + encoded(sql));
        try (Statement statement = connection.createStatement()) {
            statement.setQueryTimeout(QUERY_TIMEOUT_SECONDS);
            statement.setMaxRows(MAX_RESULT_ROWS);
            try (ResultSet result = statement.executeQuery(sql)) {
                ResultSetMetaData metadata = result.getMetaData();
                for (int column = 1; column <= metadata.getColumnCount(); column++) {
                    System.out.println(String.join("\t", "META", label, Integer.toString(column),
                        Integer.toString(metadata.getColumnType(column)),
                        encoded(metadata.getColumnTypeName(column)),
                        encoded(metadata.getColumnClassName(column)),
                        Integer.toString(metadata.getPrecision(column)),
                        Integer.toString(metadata.getScale(column)),
                        Integer.toString(metadata.isNullable(column))));
                }
                int row = 0;
                while (result.next()) {
                    row++;
                    for (int column = 1; column <= metadata.getColumnCount(); column++) {
                        Object value = result.getObject(column);
                        System.out.println(String.join("\t", "CELL", label, Integer.toString(row),
                            Integer.toString(column), value == null ? "NULL" : value.getClass().getName(),
                            value == null ? "" : encoded(value.toString())));
                    }
                }
                System.out.println("DONE\t" + label + "\t" + row);
            }
        } catch (SQLException error) {
            System.out.println(String.join("\t", "ERROR", label,
                String.valueOf(error.getSQLState()), Integer.toString(error.getErrorCode()),
                encoded(error.getMessage())));
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1 || !Files.isRegularFile(Path.of(args[0]))) {
            throw new IllegalArgumentException("Expected one existing original scalar fixture");
        }
        authorizedDatabase = Path.of(args[0]).toRealPath();
        try (Database database = new ReadOnlyOpener().open(authorizedDatabase.toFile(), null)) {
            for (TableMetaData table : database.newTableMetaDataIterable()) {
                if (table.isLinked()) {
                    throw new IOException("Linked table definitions are outside this original fixture probe");
                }
            }
            if (!database.getQueries().isEmpty()) {
                throw new IOException("Stored query definitions are outside this original fixture probe");
            }
        }

        Properties properties = new Properties();
        properties.setProperty("jackcessOpener", ReadOnlyOpener.class.getName());
        properties.setProperty("memory", "true");
        properties.setProperty("immediatelyReleaseResources", "true");
        properties.setProperty("allowRemoteLinks", "false");
        String url = "jdbc:ucanaccess://" + Path.of(args[0]).toAbsolutePath();

        try (Connection connection = DriverManager.getConnection(url, properties)) {
            connection.setReadOnly(true);
            query(connection, "count-star", "SELECT Count(*) AS V FROM ScalarSamples");
            query(connection, "count-null", "SELECT Count(NULL) AS V FROM ScalarSamples");
            query(connection, "count-empty", "SELECT Count(*) AS V FROM ScalarSamples WHERE False");
            for (String field : List.of("ID", "Octet", "Small", "Real", "WideReal", "Money", "Amount")) {
                for (String function : List.of("Count", "Sum", "Avg", "Min", "Max", "First", "Last")) {
                    query(connection, function + "-" + field,
                        "SELECT " + function + "([" + field + "]) AS V FROM ScalarSamples");
                }
            }
            query(connection, "fractional-integer-average", "SELECT Avg(ID) AS V FROM ScalarSamples WHERE ID<=2");
            query(connection, "derived-count", "SELECT V FROM (SELECT Count(*) AS V FROM ScalarSamples) Q");
            query(connection, "grouped-count", "SELECT Flag,Count(*) AS V FROM ScalarSamples GROUP BY Flag");
            query(connection, "union-count", "SELECT Count(*) AS V FROM ScalarSamples UNION ALL SELECT CLng(1) FROM ScalarSamples WHERE ID=1");
            query(connection, "long-sum-boundary", "SELECT Sum(CLng(2147483647)) AS V FROM ScalarSamples WHERE ID<=2");
            query(connection, "domain-count", "SELECT DCount('*','ScalarSamples') AS V FROM ScalarSamples WHERE ID=1");
        }
    }
}
