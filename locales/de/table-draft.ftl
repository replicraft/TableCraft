# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = Die ungespeicherte Tabelle ist nicht mehr verfügbar.
table-draft-new-record = (Neu)
table-draft-short-text = Kurzer Text (255)
table-draft-field-name = Feldname
table-draft-invalid-name = Wählen Sie einen eindeutigen gültigen Feldnamen.
table-draft-empty-note = Speichern Sie diese Tabelle, um fortzufahren.
table-draft-save = Speichern
table-draft-save-unavailable = Diese Tabelle kann derzeit nicht gespeichert werden. Lassen Sie den Entwurf geöffnet oder verwerfen Sie ihn.
table-draft-close-title = Ungespeicherte Tabelle
table-draft-close-question = Änderungen an der ungespeicherten Tabelle speichern?
table-draft-discard = Verwerfen
table-draft-cancel = Abbrechen

table-draft-save-as = Speichern unter
table-draft-table-name = Tabellenname
table-draft-invalid-table-name = Wählen Sie einen gültigen Tabellennamen mit höchstens 64 Zeichen.
table-draft-ok = OK
table-draft-save-cancelled = Das Speichern wurde abgebrochen. Der Tabellenentwurf ist unverändert.
table-draft-save-failed = Die Tabelle konnte nicht gespeichert werden.
table-draft-save-retained = Das Speicherergebnis muss wiederhergestellt oder geprüft werden. Lassen Sie dieses Fenster geöffnet.
table-draft-save-working = Tabelle wird gespeichert…
table-draft-save-cancelling = Speichern wird abgebrochen…
table-draft-save-stale = Der Speichervorgang gehört nicht mehr zu diesem Tabellenentwurf.
table-draft-save-blocked = Nach einem ungeklärten Speichervorgang bleibt das Speichern gesperrt. Öffnen Sie die Datenbank zur Prüfung erneut oder verwerfen Sie diesen Entwurf.
table-draft-save-reopen-failed = Die Tabelle wurde gespeichert, konnte aber nicht erneut geöffnet werden. Öffnen Sie die Datenbank erneut, bevor Sie diesen Entwurf wieder speichern.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = Große Zahl
table-draft-currency = Währung
table-draft-date-time = Datum und Uhrzeit
table-draft-yes-no = Ja/Nein
table-draft-long-text = Langer Text
table-draft-lookup-relationship = Nachschlagen und Beziehung
table-draft-rich-text = Rich-Text
table-draft-attachment = Anlage
table-draft-hyperlink = Hyperlink
table-draft-calculated-field = Berechnetes Feld
table-draft-more-fields = Weitere Felder
table-draft-record-field = Neuer Datensatz { $field }
table-draft-record-schema-saved = Die Tabelle wurde gespeichert; dieser Datensatz wurde noch nicht gespeichert.

table-record-preparing = Die Tabelle wurde gespeichert. Der neue Datensatz wird vorbereitet…
table-record-retry = Neuen Datensatz erneut vorbereiten
table-record-discard = Neuen Datensatz verwerfen
table-record-invalid = Die Tabelle wurde gespeichert, aber der neue Datensatz konnte nicht vorbereitet werden. Ihre Eingaben bleiben erhalten.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = Feldoptionen für { $field }
table-draft-rename-field = Feld umbenennen
table-draft-delete-field = Feld löschen
table-draft-delete-field-title = Ungespeichertes Feld löschen
table-draft-delete-field-question = Das ungespeicherte Feld „{ $field }“ löschen?
table-draft-delete-field-warning = Dadurch werden das Feld und seine noch nicht gespeicherten Datensatzeingaben aus dieser ungespeicherten Tabelle entfernt. Die Datenbank wurde nicht geändert.
table-draft-delete-field-confirm = Feld löschen

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = Bearbeiten Sie Namen, Typen und unterstützte Feldeigenschaften dieser ungespeicherten Tabelle. Andere Eigenschaften sind noch nicht verfügbar.
table-draft-design-name = Feldname für { $field }
table-draft-design-type = Datentyp für { $field }
table-draft-design-input-help = Noch nicht gespeicherte Datensatzeingaben bleiben bei einem Typwechsel unverändert. Vor der Prüfung und Übernahme müssen sie für den gewählten Typ gültig sein.
table-draft-design-protected = Der ursprüngliche AutoWert-Primärschlüssel kann nicht geändert werden.
table-draft-design-add-field = Feld hinzufügen

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = Nicht festgelegt
table-draft-property-no = Nein
table-draft-property-yes = Ja
table-draft-property-type-default = Typstandard
table-draft-property-absent = Kein Standardwert
table-draft-property-null = NULL
table-draft-property-literal = Literal
table-draft-property-literal-input = Literaler Standardwert
table-draft-property-invalid-size = Geben Sie eine ganze Zahl zwischen 1 und 255 ein.
table-draft-property-input-limit = Die Eigenschaftseingabe überschreitet ihr Limit. Der bisherige Text bleibt erhalten.
table-draft-property-invalid = Korrigieren Sie ungültige Feldeigenschaften vor dem Speichern.
table-draft-property-help = Nur Konstanten: Text ohne SQL-Anführungszeichen, Zahlen mit Dezimalpunkt, true/false oder Datum und Uhrzeit im ISO-Format eingeben. Formeln werden nicht ausgewertet. Eingabe: höchstens 4096 UTF-8-Bytes; gespeicherter Standardwert: höchstens 255 UTF-16-Einheiten einschließlich hinzugefügter und maskierter Anführungszeichen.

table-draft-number = Zahl
table-draft-number-byte = Byte
table-draft-number-integer = Ganzzahl
table-draft-number-single = Single
table-draft-number-double = Double
table-draft-number-decimal = Dezimal
table-draft-number-replication-id = Replikations-ID
table-draft-number-precision = Genauigkeit
table-draft-number-scale = Skalierung
table-draft-number-invalid-decimal = Die Genauigkeit muss 1–28 und die Skalierung 0 bis zur Genauigkeit betragen. Korrigieren Sie beide Werte vor dem Speichern.
table-draft-number-guid-default = Literale Standardwerte für Replikations-ID sind noch nicht verfügbar. Wählen Sie einen anderen Modus; der eingegebene Text bleibt erhalten.

table-draft-design-description = Beschreibung für { $field }
table-draft-description-invalid = Die Feldbeschreibung ist zu lang.

table-draft-no-key-title = Kein Primärschlüssel
table-draft-no-key-question = Es ist kein Primärschlüssel definiert. Einen AutoWert-Primärschlüssel erstellen?
table-draft-no-key-failed = Der Primärschlüssel konnte nicht hinzugefügt werden. Behalten Sie den Entwurf oder entfernen Sie ein Feld.

table-draft-design-new-name = Name des neuen Felds
table-draft-design-new-type = Datentyp des neuen Felds

table-draft-field-limit = Access-Tabellen unterstützen bis zu 255 Felder einschließlich des Primärschlüssels.
