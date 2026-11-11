# TableCraft UI: collections.

collections-title = Collection contents
collections-attachments-title = Attachments
collections-target = { $table } · { $column } · parent { $parent }
collections-reading = Reading stored contents…
collections-attachment-help = Stored metadata only. Attachment bytes remain encoded; nothing is opened or exported.
collections-history-help = Stored order and original timestamp bits; current Memo text is separate.
collections-multivalue = multivalue
collections-attachment = attachment
collections-history = stored history
collections-assigned = Assigned { $kind } collection; { $count } complete { $count ->
    [one] member
   *[other] members
    }. NULL payloads count as members
collections-unassigned = No assigned { $kind } collection
collections-encoded-context = { $context }; payloads remain encoded and are not opened
collections-history-context = { $context }; stored order retained; current Memo text is separate
collections-stored-version = stored version

collections-mock-error = Synthetic collection read error. No payload was opened or exported.

collections-stale-view = The collection no longer belongs to this view.
collections-absent-parent = No parent record
collections-query-target = Query result · { $column } · row { $row }

collections-attachment-summary = { $count } { $count ->
    [one] attachment
   *[other] attachments
    }

collections-history-summary = { $count } { $count ->
    [one] stored version
   *[other] stored versions
    }
