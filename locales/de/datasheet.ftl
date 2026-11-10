# TableCraft UI: datasheet.

datasheet-column-count = Spalten: { $count }
datasheet-previous-page = ‹ Vorherige Seite
datasheet-next-page = Nächste Seite ›
datasheet-row-range = Zeilen { $first }–{ $last }
datasheet-no-rows = Keine Zeilen.
datasheet-field-name = Feldname: { $name }
datasheet-select-record = Datensatz { $number } auswählen
datasheet-copy-record = Angezeigten Datensatz kopieren
datasheet-record-selected-help = Datensatz { $number } ist ausgewählt. Kopiert werden alle angezeigten Felder und Sammlungsvorschauen, keine untergeordneten Nutzdaten.
datasheet-select-record-help = Datensatz { $number } auswählen. Kopiert werden alle angezeigten Felder und Sammlungsvorschauen, keine untergeordneten Nutzdaten.
datasheet-view-contents = Inhalt anzeigen
datasheet-edit-cell = Zelle bearbeiten…
datasheet-cell-context = { $column }, Zeile { $row }: { $text }
datasheet-cell-details = { $column }, Zeile { $row }: { $text }; { $context }
datasheet-copy-cell-help = Kopieren übernimmt den vollständigen Zellenwert.
datasheet-copy-incomplete = Der ausgewählte Datensatz enthält nicht alle angezeigten Felder.
datasheet-copy-limit = Der angezeigte Datensatz ist zu groß zum Kopieren.
datasheet-copy-allocation = Der Zwischenablagepuffer konnte nicht angelegt werden.
datasheet-copy-encoding = Der angezeigte Datensatz konnte nicht für die Zwischenablage codiert werden.
datasheet-record = Datensatz:
datasheet-first = Erster
datasheet-previous = Vorheriger
datasheet-next = Nächster
datasheet-go-record = Gehe zu Datensatz
datasheet-record-input-help = Geben Sie eine ganze Datensatznummer zwischen 1 und 4294967296 ein.
datasheet-wrong-source = Die zurückgegebene Seite gehört zu einer anderen Quelle.
datasheet-wrong-offset = Die zurückgegebene Seite beginnt an einer anderen Datensatzposition.
datasheet-missing-column = Die angeforderte Spalte fehlt auf der zurückgegebenen Seite.
datasheet-missing-record = An dieser Position gibt es in der Momentaufnahme keinen Datensatz.
datasheet-incomplete-page = Die zurückgegebene Seite enthält den angeforderten Datensatz nicht.
datasheet-badge-stored = gespeichert
datasheet-badge-null = NULL
datasheet-badge-list = Liste
datasheet-badge-encoded = codiert
datasheet-badge-history = Verlauf
datasheet-null-context = NULL-Wert
datasheet-stored-context = Gespeichertes Berechnungsergebnis; keine Formel ausgewertet; Aktualität unbekannt
datasheet-stored-null-context = Gespeichertes NULL-Berechnungsergebnis; keine Formel ausgewertet; Aktualität unbekannt
datasheet-control-uninspected = Die Feldanzeige wurde nicht geprüft; der gespeicherte Wert wird angezeigt
datasheet-control-absent = Kein Anzeigesteuerelement deklariert; der gespeicherte Wert wird angezeigt
datasheet-control-unsupported = Die Feldanzeige wird nicht unterstützt; der gespeicherte Wert wird angezeigt
datasheet-control-textbox = Dieses Feld verwendet ein Textfeld
datasheet-control-lookup = Nachschlageanzeigen werden noch nicht unterstützt; der gespeicherte Wert wird angezeigt
datasheet-context-pair = { $base }; { $presentation }

datasheet-go = Los

datasheet-null-value = NULL
datasheet-close-table = Tabelle schließen
datasheet-column-menu = Spaltenmenü { $column }

datasheet-last = Letzter

datasheet-value-preview-truncated = Vorschau gekürzt. Kopieren übernimmt den vollständigen Anzeigewert.

datasheet-row-identity-invalid = Die Datensatzidentitäten stimmen nicht mit dieser Seite überein.
datasheet-filter-page-unavailable = Hier sind nur die geladenen gefilterten Datensätze verfügbar.

datasheet-click-to-add = Zum Hinzufügen klicken

datasheet-add-field-unavailable = Das Hinzufügen von Feldern ist noch nicht verfügbar.

datasheet-record-position = Datensatznummer

datasheet-record-of = von { $count }

datasheet-count-unknown = ?

datasheet-search-records = Datensätze suchen

datasheet-no-filter = Kein Filter

datasheet-filtered = Gefiltert
datasheet-page-tools = Seitenwerkzeuge

# Saved-table field addition review and checked reopen.
datasheet-field-title = Feld
datasheet-field-inspecting = Die Möglichkeit zum Hinzufügen eines Felds wird geprüft…
datasheet-field-reviewing = Das neue Feld wird geprüft…
datasheet-field-reopening = Die gespeicherte Tabelle wird erneut geöffnet…
datasheet-field-retry-inspection = Feldprüfung wiederholen

# Saved field close confirmation
datasheet-field-close-title = Feldänderungen speichern?
datasheet-field-close-question = Das neue Feld „{ $column }“ in der Tabelle „{ $table }“ speichern?
datasheet-field-close-stale = Der Feldname oder die Tabelle wurde geändert. Schließen Sie diese Abfrage und versuchen Sie es erneut.
datasheet-field-close-unavailable = Dieses Feld kann momentan nicht gespeichert werden.
datasheet-field-close-save = Speichern
datasheet-field-close-discard = Nicht speichern
datasheet-field-close-cancel = Abbrechen

# Interrupted readonly field eligibility observation.
datasheet-field-inspection-interrupted = Die Feldprüfung wurde unterbrochen. Wiederholen Sie die Prüfung dieser Tabelle.

# Known saved field after failed checked reopening.
datasheet-field-saved-warning = Das Feld wurde gespeichert, aber die Datenbank konnte nicht verifiziert werden. Öffnen Sie die Datenbank vor dem nächsten Speichern erneut.
datasheet-field-abandon-title = Feldentwurf schließen?
datasheet-field-abandon-question = Das Feld wurde gespeichert. Das Schließen dieses Entwurfs macht die Speicherung nicht rückgängig.
datasheet-field-abandon-discard = Entwurf schließen

# Leave failed checked reopening without undoing an acknowledged field save.
datasheet-field-return-draft = Zum Feldentwurf zurückkehren
