# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = Aktualisierungsabfrage
query-action-update-reviewing = Aktualisierungsabfrage wird geprüft…
query-action-table = Tabelle: { $table }
query-action-matched = Gefundene Datensätze: { $count }
query-action-update-changed = Datensätze mit tatsächlichen Änderungen: { $count }
query-action-update-confirmation = Diese geprüften Änderungen auf die ursprüngliche Datenbank anwenden?
query-action-no-undo = Dieser Vorgang kann nicht rückgängig gemacht werden.
query-action-yes = Ja
query-action-no = Nein
query-action-invalid-review = Die geprüfte Aktion stimmt nicht mehr mit diesem SQL-Entwurf überein.
query-action-delete-title = Löschabfrage
query-action-delete-reviewing = Löschabfrage wird geprüft…
query-action-delete-deleted = Zu löschende Datensätze: { $count }
query-action-delete-confirmation = Diese geprüften Datensätze dauerhaft aus der ursprünglichen Datenbank löschen?
query-action-insert-title = Anfügeabfrage
query-action-insert-reviewing = Anfügeabfrage wird geprüft…
query-action-insert-inserted = Anzufügende Datensätze: { $count }
query-action-insert-confirmation = Die geprüften Datensätze an die ursprüngliche Datenbank anfügen?

## Reviewed SQL schema changes
query-action-schema-title = Datendefinitionsabfrage
query-action-schema-reviewing = Schemaabfrage wird geprüft…
query-action-schema-confirmation = Diese geprüfte Änderung der Datenbankstruktur anwenden?
query-action-schema-create-fields = Zu erstellende Felder: { $count }
query-action-schema-drop-records = Datensätze in der zu löschenden Tabelle: { $count }
query-action-schema-drop-field = Zu löschendes Feld: { $column }
query-action-schema-create-confirmation = Diese geprüfte Tabelle in der ursprünglichen Datenbank erstellen?
query-action-schema-drop-table-confirmation = Diese geprüfte Tabelle und ihre Datensätze dauerhaft aus der ursprünglichen Datenbank löschen?
query-action-schema-drop-column-confirmation = Dieses geprüfte Feld und seine Werte dauerhaft aus der ursprünglichen Datenbank löschen?
query-action-schema-close-target = Schließen Sie die Zieltabelle, bevor Sie diese Schemaabfrage ausführen.
query-action-schema-reopen-failed = Die Schemaänderung wurde gespeichert, aber die Datenbank konnte nicht erneut geöffnet werden. Wiederholen Sie die Abfrage nicht.
query-action-schema-retry-reopen = Erneut öffnen
query-action-schema-retained-results = Vorherige Ergebnisse bleiben erhalten. Öffnen Sie die gespeicherte Datenbank erneut, um ihre Struktur zu aktualisieren; wiederholen Sie die Abfrage nicht.

query-action-schema-make-table-confirmation = Diese Tabelle erstellen und alle ausgewählten Datensätze in einem Vorgang einfügen?
query-action-schema-make-table-records = Ausgewählte Datensätze zum Einfügen: { $count }

query-action-schema-index-name = Index: { $name }
query-action-schema-index-field = Feld: { $name }
query-action-schema-index-ascending = Reihenfolge: Aufsteigend
query-action-schema-index-descending = Reihenfolge: Absteigend
query-action-schema-index-source-rows = Zu indizierende vorhandene Datensätze: { $count }
query-action-schema-index-confirmation = Diesen nicht eindeutigen Index in der ursprünglichen Datenbank erstellen?
