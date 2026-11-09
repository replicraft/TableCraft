# Имена сохранённых полей: проверка, изменение и проверенное повторное открытие.

field-rename-title = Переименовать поле
field-rename-name = Имя поля
field-rename-design-help = Используйте команду «{ field-rename-title }», чтобы предложить новое имя поля. Остальные свойства поля доступны только для чтения.
field-rename-original = Исходное поле: { $column }
field-rename-proposed = Новое имя: { $column }
field-rename-inspecting = Проверяется, можно ли переименовать это поле…
field-rename-inspection-interrupted = Проверка поля прервана. Повторите проверку этого поля.
field-rename-retry-inspection = Повторить проверку поля
field-rename-source-unavailable = Переименование этого поля недоступно для данной базы данных.
field-rename-profile =
    { $units ->
        [one] Пока поддерживаются только имена полей из символов ASCII длиной ровно { $units } символ.
        [few] Пока поддерживаются только имена полей из символов ASCII длиной ровно { $units } символа.
        [many] Пока поддерживаются только имена полей из символов ASCII длиной ровно { $units } символов.
       *[other] Пока поддерживаются только имена полей из символов ASCII длиной ровно { $units } символа.
    }
field-rename-reviewing = Проверяется имя поля…
field-rename-reopening = Переименованное поле открывается заново и проверяется…
field-rename-unchanged-reopening = Проверяется база данных без изменений…
field-rename-unchanged = Изменение имени поля не записывалось.
field-rename-cancel = Отменить переименование
field-rename-return = Вернуться к редактору имени
field-rename-close-title = Изменения имени поля
field-rename-close-question = Сохранить изменения имени поля { $column } в таблице { $table } перед закрытием?
field-rename-close-save = Сохранить
field-rename-close-discard = Не сохранять
field-rename-close-cancel = Отмена
field-rename-close-unavailable = Сохранение этого имени недоступно. Откажитесь от изменений или вернитесь к редактору имени.
field-rename-close-stale = Новое имя изменилось. Отмените закрытие и попробуйте ещё раз.
field-rename-close-confirm = Закрыть редактор имени? Это не отменит изменения, уже записанные в файл.
field-rename-warning-saved = Имя поля записано, но проверить базу данных не удалось. Откройте её заново, прежде чем вносить другие изменения.
field-rename-warning-unchanged = Изменение имени поля не записывалось, но проверить базу данных не удалось. Откройте её заново, прежде чем вносить другие изменения.
field-rename-close-editor = Закрыть редактор имени
