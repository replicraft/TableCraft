//! Access identifier classification, independent of upstream multi-dialect keywords.
// Keyword facts: https://learn.microsoft.com/en-us/office/client-developer/access/desktop-database-reference/sql-reserved-words
// This is SQL name classification, not Access UI/VBA name binding.
pub(super) fn unreserved(value: &str) -> bool {
    !matches!(
        value.to_ascii_uppercase().as_str(),
        "ABSOLUTE" | "ADD" | "ADMINDB" | "ALL" | "ALPHANUMERIC" | "ALTER" | "AND"
        | "ANY" | "ARE" | "AS" | "ASC" | "ASSERTION" | "AUTHORIZATION" | "AUTOINCREMENT" | "AVG"
        | "BEGIN" | "BETWEEN" | "BINARY" | "BIT" | "BIT_LENGTH" | "BOOLEAN" | "BOTH" | "BY" | "BYTE"
        | "CASCADE" | "CATALOG" | "CHAR" | "CHARACTER" | "CHAR_LENGTH" | "CHARACTER_LENGTH"
        | "CHECK" | "CLOSE" | "CLUSTERED" | "COALESCE" | "COLLATE" | "COLLATION" | "COLUMN"
        | "COMMIT" | "COMP" | "COMPRESSION" | "CONNECT" | "CONNECTION" | "CONSTRAINT"
        | "CONSTRAINTS" | "CONTAINER" | "CONTAINS" | "CONVERT" | "COUNT" | "COUNTER" | "CREATE"
        | "CURRENCY" | "CURRENT_DATE" | "CURRENT_TIME" | "CURRENT_TIMESTAMP" | "CURRENT_USER" | "CURSOR"
        | "DATABASE" | "DATE" | "DATETIME" | "DAY" | "DEC" | "DECIMAL" | "DECLARE" | "DELETE"
        | "DESC" | "DISALLOW" | "DISCONNECT" | "DISTINCT" | "DISTINCTROW" | "DOMAIN" | "DOUBLE" | "DROP"
        | "EQV" | "EXCLUSIVECONNECT" | "EXEC" | "EXECUTE" | "EXISTS" | "EXTRACT" | "FALSE" | "FETCH"
        | "FIRST" | "FLOAT" | "FLOAT8" | "FLOAT4" | "FOREIGN" | "FROM" | "GENERAL" | "GRANT"
        | "GROUP" | "GUID" | "HAVING" | "HOUR" | "IDENTITY" | "IEEEDOUBLE" | "IEEESINGLE" | "IGNORE"
        | "IMAGE" | "IMP" | "IN" | "INDEX" | "INDEXCREATEDB" | "INNER" | "INPUT" | "INSENSITIVE"
        | "INSERT" | "INT" | "INTEGER" | "INTEGER4" | "INTEGER1" | "INTEGER2" | "INTERVAL" | "INTO"
        | "IS" | "ISOLATION" | "JOIN" | "KEY" | "LANGUAGE" | "LAST" | "LEFT" | "LEVEL" | "LIKE"
        | "LOGICAL" | "LOGICAL1" | "LONG" | "LONGBINARY" | "LONGCHAR" | "LONGTEXT" | "LOWER"
        | "MATCH" | "MAX" | "MEMO" | "MIN" | "MINUTE" | "MOD" | "MONEY" | "MONTH" | "NATIONAL"
        | "NCHAR" | "NONCLUSTERED" | "NOT" | "NTEXT" | "NULL" | "NUMBER" | "NUMERIC" | "NVARCHAR"
        | "OCTET_LENGTH" | "OLEOBJECT" | "ON" | "OPEN" | "OPTION" | "OR" | "ORDER" | "OUTER"
        | "OUTPUT" | "OWNERACCESS" | "PAD" | "PARAMETERS" | "PARTIAL" | "PASSWORD" | "PERCENT"
        | "PIVOT" | "POSITION" | "PRECISION" | "PREPARE" | "PRIMARY" | "PRIVILEGES" | "PROC"
        | "PROCEDURE" | "PUBLIC" | "REAL" | "REFERENCES" | "RESTRICT" | "REVOKE" | "RIGHT" | "ROLLBACK"
        | "SCHEMA" | "SECOND" | "SELECT" | "SELECTSCHEMA" | "SELECTSECURITY" | "SET" | "SHORT"
        | "SINGLE" | "SIZE" | "SMALLDATETIME" | "SMALLINT" | "SMALLMONEY" | "SOME" | "SPACE"
        | "SQL" | "SQLCODE" | "SQLERROR" | "SQLSTATE" | "STDEV" | "STDEVP" | "STRING" | "SUBSTRING"
        | "SUM" | "SYSNAME" | "SYSTEM_USER" | "TABLE" | "TABLEID" | "TEMPORARY" | "TEXT" | "TIME"
        | "TIMESTAMP" | "TIMEZONE_HOUR" | "TIMEZONE_MINUTE" | "TINYINT" | "TO" | "TOP" | "TRAILING"
        | "TRANSACTION" | "TRANSFORM" | "TRANSLATE" | "TRANSLATION" | "TRIM" | "TRUE" | "UNION"
        | "UNIQUE" | "UNIQUEIDENTIFIER" | "UNKNOWN" | "UPDATE" | "UPDATEIDENTITY" | "UPDATEOWNER"
        | "UPDATESECURITY" | "UPPER" | "USAGE" | "USER" | "USING" | "VALUE" | "VALUES" | "VAR"
        | "VARBINARY" | "VARCHAR" | "VARP" | "VARYING" | "VIEW" | "WHEN" | "WHENEVER" | "WHERE"
        | "WITH" | "WORK" | "XOR" | "YEAR" | "YESNO" | "ZONE"
        // Keep unsupported foreign grammar from becoming implicit aliases or calls.
        | "CASE" | "CAST" | "LIMIT" | "OFFSET" | "EXCEPT" | "INTERSECT" | "FULL" | "CROSS"
        | "APPLY" | "NATURAL" | "OVER" | "WINDOW" | "RETURNING"
    )
}
