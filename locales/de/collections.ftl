# TableCraft UI: collections.

collections-title = Sammlungsinhalt
collections-attachments-title = Anlagen
collections-target = { $table } · { $column } · übergeordnet { $parent }
collections-reading = Gespeicherter Inhalt wird gelesen…
collections-attachment-help = Nur gespeicherte Metadaten. Anlagen bleiben codiert; nichts wird geöffnet oder exportiert.
collections-history-help = Gespeicherte Reihenfolge und ursprüngliche Zeitstempelbits; aktueller Memo-Text ist getrennt.
collections-multivalue = mehrwertig
collections-attachment = Anlage
collections-history = gespeicherter Verlauf
collections-assigned = Zugewiesene Sammlung: { $kind }; vollständige Elemente: { $count }. NULL-Nutzdaten zählen als Elemente
collections-unassigned = Keine Sammlung zugewiesen: { $kind }
collections-encoded-context = { $context }; Nutzdaten bleiben codiert und werden nicht geöffnet
collections-history-context = { $context }; gespeicherte Reihenfolge bleibt erhalten; aktueller Memo-Text ist getrennt
collections-stored-version = gespeicherte Version

collections-mock-error = Simulierter Lesefehler der Sammlung. Es wurden keine Nutzdaten geöffnet oder exportiert.

collections-stale-view = Die Sammlung gehört nicht mehr zu dieser Ansicht.
collections-absent-parent = Kein übergeordneter Datensatz
collections-query-target = Abfrageergebnis · { $column } · Zeile { $row }

collections-attachment-summary = { $count } { $count ->
    [one] Anlage
   *[other] Anlagen
    }

collections-history-summary = { $count } { $count ->
    [one] gespeicherte Version
   *[other] gespeicherte Versionen
    }
