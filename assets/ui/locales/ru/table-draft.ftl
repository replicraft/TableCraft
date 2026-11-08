# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = Несохранённая таблица больше недоступна.
table-draft-new-record = (Новая)
table-draft-short-text = Краткий текст (255)
table-draft-field-name = Имя поля
table-draft-invalid-name = Выберите допустимое уникальное имя поля.
table-draft-empty-note = Сохраните эту таблицу, чтобы продолжить.
table-draft-save = Сохранить
table-draft-save-unavailable = Сохранение этой таблицы сейчас недоступно. Оставьте черновик открытым или отмените изменения.
table-draft-close-title = Несохранённая таблица
table-draft-close-question = Сохранить изменения в несохранённой таблице?
table-draft-discard = Отменить изменения
table-draft-cancel = Отмена

table-draft-save-as = Сохранение
table-draft-table-name = Имя таблицы
table-draft-invalid-table-name = Выберите допустимое имя таблицы длиной до 64 знаков.
table-draft-ok = ОК
table-draft-save-cancelled = Сохранение отменено. Черновик таблицы не изменён.
table-draft-save-failed = Не удалось сохранить таблицу.
table-draft-save-retained = Результат сохранения требует восстановления или проверки. Оставьте это окно открытым.
table-draft-save-working = Сохранение таблицы…
table-draft-save-cancelling = Отмена сохранения…
table-draft-save-stale = Сохранение больше не относится к этому черновику таблицы.
table-draft-save-blocked = Сохранение заблокировано из-за неясного результата. Снова откройте базу для проверки или отмените этот черновик.
table-draft-save-reopen-failed = Таблица сохранена, но повторное открытие не завершено. Откройте базу заново, прежде чем снова сохранять этот черновик.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = Большое число
table-draft-currency = Денежный
table-draft-date-time = Дата и время
table-draft-yes-no = Логический
table-draft-long-text = Длинный текст
table-draft-lookup-relationship = Подстановка и отношение
table-draft-rich-text = Форматированный текст
table-draft-attachment = Вложение
table-draft-hyperlink = Гиперссылка
table-draft-calculated-field = Вычисляемое поле
table-draft-more-fields = Другие поля
table-draft-record-field = Новая запись: { $field }
table-draft-record-schema-saved = Таблица сохранена; эта запись ещё не сохранена.

table-record-preparing = Таблица сохранена. Подготовка новой записи…
table-record-retry = Повторить подготовку записи
table-record-discard = Отменить новую запись
table-record-invalid = Таблица сохранена, но подготовить новую запись не удалось. Введённые данные сохранены.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = Параметры поля { $field }
table-draft-rename-field = Переименовать поле
table-draft-delete-field = Удалить поле
table-draft-delete-field-title = Удаление несохранённого поля
table-draft-delete-field-question = Удалить несохранённое поле «{ $field }»?
table-draft-delete-field-warning = Поле и введённые для него данные записи будут удалены из этой несохранённой таблицы. База данных не изменена.
table-draft-delete-field-confirm = Удалить поле

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = Редактируйте имена, типы и поддерживаемые свойства полей этой несохранённой таблицы. Остальные свойства пока недоступны.
table-draft-design-name = Имя поля { $field }
table-draft-design-type = Тип данных поля { $field }
table-draft-design-input-help = При смене типа введённые данные записи сохраняются без изменений. Перед проверкой и применением они должны соответствовать выбранному типу.
table-draft-design-protected = Исходный первичный ключ типа «Счётчик» нельзя изменить.
table-draft-design-add-field = Добавить поле

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = Не задано
table-draft-property-no = Нет
table-draft-property-yes = Да
table-draft-property-type-default = По умолчанию для типа
table-draft-property-absent = Без значения по умолчанию
table-draft-property-null = NULL
table-draft-property-literal = Литерал
table-draft-property-literal-input = Литерал по умолчанию
table-draft-property-invalid-size = Введите целое число от 1 до 255.
table-draft-property-input-limit = Ввод свойства превышает ограничение. Предыдущий текст сохранён.
table-draft-property-invalid = Исправьте недопустимые свойства полей перед сохранением.
table-draft-property-help = Только константы: текст без кавычек SQL, числа с десятичной точкой, true/false или дата и время в формате ISO. Формулы не вычисляются. Ввод ограничен 4096 байтами UTF-8; сохранённое значение по умолчанию — 255 единицами UTF-16, включая добавленные и экранированные кавычки.

table-draft-number = Числовой
table-draft-number-byte = Байт
table-draft-number-integer = Целое
table-draft-number-single = Одинарное
table-draft-number-double = Двойное
table-draft-number-decimal = Десятичное
table-draft-number-replication-id = Код репликации
table-draft-number-precision = Точность
table-draft-number-scale = Масштаб
table-draft-number-invalid-decimal = Точность должна быть от 1 до 28, а масштаб — от 0 до точности. Исправьте оба значения перед сохранением.
table-draft-number-guid-default = Литеральные значения по умолчанию для кода репликации пока недоступны. Выберите другой режим; введённый текст сохранится.

table-draft-design-description = Описание поля { $field }
table-draft-description-invalid = Описание поля слишком длинное.
