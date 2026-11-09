# TableCraft UI: datasheet.

datasheet-column-count = Столбцов: { $count }
datasheet-previous-page = ‹ Предыдущая страница
datasheet-next-page = Следующая страница ›
datasheet-row-range = Строки { $first }–{ $last }
datasheet-no-rows = Нет строк.
datasheet-field-name = Имя поля: { $name }
datasheet-select-record = Выделить запись { $number }
datasheet-copy-record = Копировать показанную запись
datasheet-record-selected-help = Выделена запись { $number }. Копируются все показанные поля и сводки коллекций, без дочерних данных.
datasheet-select-record-help = Выделить запись { $number }. Копируются все показанные поля и сводки коллекций, без дочерних данных.
datasheet-view-contents = Просмотр содержимого
datasheet-edit-cell = Изменить ячейку…
datasheet-cell-context = { $column }, строка { $row }: { $text }
datasheet-cell-details = { $column }, строка { $row }: { $text }; { $context }
datasheet-copy-cell-help = Копируется полное значение ячейки.
datasheet-copy-incomplete = Выделенная запись содержит не все показанные поля.
datasheet-copy-limit = Показанная запись превышает ограничение буфера обмена в 1 МиБ.
datasheet-copy-allocation = Не удалось выделить память для буфера обмена.
datasheet-copy-encoding = Не удалось закодировать показанную запись для буфера обмена.
datasheet-record = Запись:
datasheet-first = Первая
datasheet-previous = Предыдущая
datasheet-next = Следующая
datasheet-go-record = Перейти к записи
datasheet-record-input-help = Введите целый номер записи от 1 до 4294967296.
datasheet-wrong-source = Полученная страница относится к другому источнику.
datasheet-wrong-offset = Полученная страница начинается с другой позиции записи.
datasheet-missing-column = Запрошенного столбца нет на полученной странице.
datasheet-missing-record = В этой позиции снимка нет записи.
datasheet-incomplete-page = Полученная страница не содержит запрошенную запись.
datasheet-badge-stored = сохранено
datasheet-badge-null = NULL
datasheet-badge-list = список
datasheet-badge-encoded = закодировано
datasheet-badge-history = журнал
datasheet-null-context = Значение NULL
datasheet-stored-context = Сохранённый вычисляемый результат; формула не вычислялась; актуальность неизвестна
datasheet-stored-null-context = Сохранённый вычисляемый результат NULL; формула не вычислялась; актуальность неизвестна
datasheet-control-uninspected = Параметры отображения поля не проверены; показано сохранённое значение
datasheet-control-absent = Элемент отображения не объявлен; показано сохранённое значение
datasheet-control-unsupported = Параметры отображения поля не поддерживаются; показано сохранённое значение
datasheet-control-textbox = Это поле использует текстовое поле
datasheet-control-lookup = Отображение подстановки ещё не поддерживается; показано сохранённое значение
datasheet-context-pair = { $base }; { $presentation }

datasheet-go = Перейти

datasheet-null-value = NULL
datasheet-close-table = Закрыть таблицу
datasheet-column-menu = Меню столбца { $column }

datasheet-last = Последняя

datasheet-value-preview-truncated = Предпросмотр сокращён. Копирование сохраняет полное отображаемое значение.

datasheet-row-identity-invalid = Идентификаторы записей не соответствуют этой странице.
datasheet-filter-page-unavailable = Здесь доступны только загруженные отфильтрованные записи.

datasheet-click-to-add = Щёлкните для добавления

datasheet-add-field-unavailable = Добавление полей пока недоступно.

datasheet-record-position = Номер записи

datasheet-record-of = из { $count }

datasheet-count-unknown = ?

datasheet-search-records = Поиск записей

datasheet-no-filter = Без фильтра

datasheet-filtered = Отфильтровано
datasheet-page-tools = Инструменты страниц

# Saved-table Number field review and checked reopen.
datasheet-number-inspecting = Проверяется возможность добавления числового поля…
datasheet-number-reviewing = Проверяется новое числовое поле…
datasheet-number-reopening = Сохранённая таблица открывается повторно…
datasheet-number-retry-inspection = Повторить проверку возможности добавления поля

# Saved Number field close confirmation
datasheet-number-close-title = Сохранить изменения поля?
datasheet-number-close-question = Сохранить новое поле «{ $column }» в таблице «{ $table }»?
datasheet-number-close-stale = Имя поля или таблица изменились. Закройте это окно и повторите попытку.
datasheet-number-close-unavailable = Сейчас это поле сохранить нельзя.
datasheet-number-close-save = Сохранить
datasheet-number-close-discard = Не сохранять
datasheet-number-close-cancel = Отмена

# Interrupted readonly field eligibility observation.
datasheet-number-inspection-interrupted = Проверка поля прервана. Повторите проверку этой таблицы.

# Known saved field after failed checked reopening.
datasheet-number-saved-warning = Поле сохранено, но проверить базу данных не удалось. Перед следующим сохранением откройте базу заново.
datasheet-number-abandon-title = Закрыть черновик поля?
datasheet-number-abandon-question = Поле сохранено. Закрытие черновика не отменяет сохранение.
datasheet-number-abandon-discard = Закрыть черновик

# Leave failed checked reopening without undoing an acknowledged field save.
datasheet-number-return-draft = Вернуться к черновику поля
