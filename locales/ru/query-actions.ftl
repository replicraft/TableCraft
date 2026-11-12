# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = Запрос на обновление
query-action-update-reviewing = Проверка запроса на обновление…
query-action-table = Таблица: { $table }
query-action-matched = Найдено записей: { $count }
query-action-update-changed = Записей с фактическими изменениями: { $count }
query-action-update-confirmation = Применить проверенные изменения к исходной базе данных?
query-action-no-undo = Эту операцию нельзя отменить.
query-action-yes = Да
query-action-no = Нет
query-action-invalid-review = Проверенное действие больше не соответствует этому черновику SQL.
query-action-delete-title = Запрос на удаление
query-action-delete-reviewing = Проверка запроса на удаление…
query-action-delete-deleted = Записей для удаления: { $count }
query-action-delete-confirmation = Безвозвратно удалить эти проверенные записи из исходной базы данных?
query-action-insert-title = Запрос на добавление
query-action-insert-reviewing = Проверка запроса на добавление…
query-action-insert-inserted = Записей для добавления: { $count }
query-action-insert-confirmation = Добавить проверенные записи в исходную базу данных?

## Reviewed SQL schema changes
query-action-schema-title = Запрос определения данных
query-action-schema-reviewing = Проверка запроса изменения структуры…
query-action-schema-confirmation = Применить это проверенное изменение структуры базы данных?
query-action-schema-create-fields = Создаваемые поля: { $count }
query-action-schema-drop-records = Записей в удаляемой таблице: { $count }
query-action-schema-drop-field = Удаляемое поле: { $column }
query-action-schema-create-confirmation = Создать эту проверенную таблицу в исходной базе данных?
query-action-schema-drop-table-confirmation = Безвозвратно удалить эту проверенную таблицу и её записи из исходной базы данных?
query-action-schema-drop-column-confirmation = Безвозвратно удалить это проверенное поле и его значения из исходной базы данных?
query-action-schema-close-target = Закройте целевую таблицу перед выполнением этого запроса изменения структуры.
query-action-schema-reopen-failed = Изменение структуры сохранено, но базу данных не удалось открыть заново. Не повторяйте запрос.
query-action-schema-retry-reopen = Повторить открытие
query-action-schema-retained-results = Предыдущие результаты сохранены. Повторно откройте сохранённую базу данных, чтобы обновить её структуру; не повторяйте запрос.

query-action-schema-make-table-confirmation = Создать эту таблицу и вставить все выбранные записи одной операцией?
query-action-schema-make-table-records = Выбранные записи для вставки: { $count }

query-action-schema-index-name = Индекс: { $name }
query-action-schema-index-field = Поле: { $name }
query-action-schema-index-ascending = Порядок: По возрастанию
query-action-schema-index-descending = Порядок: По убыванию
query-action-schema-index-source-rows = Существующие записи для индексации: { $count }
query-action-schema-index-confirmation = Создать этот неуникальный индекс в исходной базе данных?

query-action-schema-add-confirmation = Добавить это поле в исходную таблицу?

query-action-schema-drop-index-name = Удаляемый индекс: { $name }
query-action-schema-drop-index-confirmation = Безвозвратно удалить этот индекс из исходной базы данных? Значения записей останутся без изменений.
