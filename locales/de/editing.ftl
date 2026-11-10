# TableCraft UI: editing.

editing-title = Offline-Zellenbearbeitung
editing-exclusive-help = Exklusive Offline-Bearbeitung. Schließen Sie diese Datei vor dem Anwenden in Access und anderen Anwendungen.
editing-checking = Die Zelle wird geprüft und ihr ursprünglicher Wert gelesen…
editing-original = Ursprünglicher gespeicherter Wert
editing-replacement = Ersatz
editing-use-null = NULL verwenden (vorbehaltlich Validierung)
editing-validating = Die vollständige Änderung wird geprüft…
editing-reviewed = Geprüfte Änderung
editing-before = Vorher
editing-after = Nachher
editing-changed-rows = Geänderte Zeilen: { $count }
editing-apply = Auf Originaldatei anwenden
editing-review = Änderung prüfen
editing-resolving = Zurückbehaltene Speicherressourcen werden geklärt. Lassen Sie das Fenster offen, bis das Ergebnis feststeht.
editing-applying = Die geprüfte Änderung wird angewendet. Lassen Sie das Fenster offen, bis das Ergebnis feststeht.
editing-cancellation-requested = Abbruch angefordert; das tatsächliche Speicherergebnis wird abgewartet.
editing-request-cancellation = Abbruch anfordern
editing-recovery-owned = Die Sitzung hält noch Wiederherstellungsressourcen. Klären Sie sie vor dem Fortfahren.
editing-recover = Speicherung wiederherstellen
editing-release = Speicherressourcen freigeben
editing-diagnosis-required = Eine Diagnose ist erforderlich; automatische Wiederherstellung und Freigabe sind nicht verfügbar.
editing-target = { $table } · { $column } · Zeile { $row }
editing-stored-value = Gespeicherter Wert

editing-mock-rejected = Die simulierte Prüfung hat diesen Beispielwert abgelehnt. Es wurde keine Datenbank geöffnet oder geändert.

editing-mock-failed = Das simulierte Speichern ist fehlgeschlagen. Es wurde keine Datenbank geöffnet oder geändert.

editing-mock-diagnosis = Das simulierte Ergebnis erfordert eine Diagnose. In dieser Vorschau gibt es keine Datenbankdatei und kein Journal.

editing-outcome-saved = Speichern abgeschlossen

editing-outcome-cancelled = Speichern abgebrochen

editing-outcome-failed = Speichervorgang erfordert Aufmerksamkeit

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = übernommen
editing-save-known-rolled-back = zurückgesetzt
editing-save-known-original = Originalabbild geprüft
editing-save-known-replacement = Ersatzabbild geprüft
editing-save-known-unknown = nicht festgestellt
editing-save-completion-applied = angewendet
editing-save-completion-recovered = wiederhergestellt
editing-save-completion-confirmed = bestätigt
editing-save-completed = Speichern abgeschlossen und Freigabe der Sperre bestätigt.
editing-save-released = Ressourcen freigegeben. Wiederherstellungsdateien können noch vorhanden sein; klären Sie dies, bevor Sie diese Datei verwenden. { $details }
editing-save-current = Ergebnis des aktuellen Vorgangs: { $outcome }.
editing-save-no-receipt = Es liegt kein Beleg für einen abgeschlossenen Vorgang vor.
editing-save-prior = Letzter abgeschlossener Vorgang: { $completion }; Ergebnis: { $outcome }.
editing-save-matched = Übereinstimmende Zeilen: { $count }.
editing-save-changed = Geänderte Zeilen: { $count }.
editing-save-failure = Das Speichern wurde nicht ordnungsgemäß abgeschlossen ({ $reason }). { $details } Nicht automatisch erneut versuchen.
editing-save-reason-stale = das angezeigte Dokument wurde ersetzt
editing-save-reason-open = die Datei konnte nicht zur exklusiven Bearbeitung geöffnet werden
editing-save-reason-source = die Datei wurde nach dem Anzeigen geändert
editing-save-reason-cancelled = Abbruch wurde angefordert
editing-save-reason-resource = eine Vorgangsgrenze wurde erreicht
editing-save-reason-planning = die Änderung konnte nicht validiert werden
editing-save-reason-intent = der Änderungsdatensatz konnte nicht vorbereitet werden
editing-save-reason-setup = die Wiederherstellungsdateien konnten nicht eingerichtet werden
editing-save-reason-prepare = das Speichern konnte nicht vorbereitet werden
editing-save-reason-commit = die Übernahme wurde nicht ordnungsgemäß abgeschlossen
editing-save-reason-recovery = die Wiederherstellung wurde nicht ordnungsgemäß abgeschlossen
editing-save-reason-finalization = der Speicherabschluss wurde nicht ordnungsgemäß beendet
editing-save-reason-release = die Freigabe der Sperre wurde nicht bestätigt
editing-save-reason-state = die Speichersitzung muss untersucht werden

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = Wahrheitswert: true oder false
editing-input-byte = Byte: 0 bis 255
editing-input-int16 = Vorzeichenbehaftete 16-Bit-Ganzzahl
editing-input-int32 = Vorzeichenbehaftete 32-Bit-Ganzzahl
editing-input-int64 = Vorzeichenbehaftete 64-Bit-Ganzzahl
editing-input-single = Single: Dezimalzahl Als Dezimaltrennzeichen "." verwenden.
editing-input-double = Double: Dezimalzahl Als Dezimaltrennzeichen "." verwenden.
editing-input-currency = Währung: Dezimalbetrag (vier Nachkommastellen) Als Dezimaltrennzeichen "." verwenden.
editing-input-decimal = Dezimalzahl Als Dezimaltrennzeichen "." verwenden.
editing-input-text = Wörtlicher Text
editing-input-binary = Binär: hexadezimale Bytepaare
editing-input-guid = GUID: kanonischer Bezeichner
editing-input-date = Klassisches DATE: lokales Datum und Uhrzeit im ISO-Format (ohne Zeitzone)
editing-input-date-extended = Erweiterte Datumszeit: lokales Datum und Uhrzeit im ISO-Format (100 ns Genauigkeit)

editing-paste-text = Text einfügen
editing-paste-help = Fügen Sie wörtlichen Text in eine ausgewählte native Zelle ein und prüfen Sie ihn. Verwenden Sie das gebietsschemaunabhängige Eingabeformat des Felds; formatierte Werte und typisierte Access-Zwischenablageformate werden nicht interpretiert. Mehrzeiliger Text bleibt in einer Zelle.
editing-paste-stale = Die Zwischenablageanforderung gehört nicht mehr zur ausgewählten Zelle. Der Text wurde verworfen.
editing-paste-multiple = Die Zwischenablage hat mehrere Texte geliefert. Es wurde nichts eingefügt.

editing-save-retained-private-journal = Nach dem Speichern bleibt ein privates Wiederherstellungsjournal auf dem Datenträger erhalten.

# An actor ended without a known current native outcome.
editing-save-uncertain = Der Speichervorgang wurde unerwartet beendet. Die Datei kann bereits geändert sein. Starten Sie TableCraft neu und öffnen Sie die Datenbank erneut, bevor Sie weitere Änderungen vornehmen. Prüfen Sie ein vorhandenes Wiederherstellungsjournal. Ein Zurücksetzen oder erneuter Versuch ist nicht bestätigt.

editing-toggle-stale = Das Kontrollkästchen entspricht nicht mehr der geprüften Zelle. Die vorgeschlagene Änderung wurde verworfen.
