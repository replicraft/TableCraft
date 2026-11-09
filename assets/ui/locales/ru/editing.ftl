# TableCraft UI: editing.

editing-title = Автономное изменение ячейки
editing-exclusive-help = Исключительное автономное изменение. Закройте файл в Access и других приложениях перед применением.
editing-checking = Проверка ячейки и чтение исходного значения…
editing-original = Исходное сохранённое значение
editing-replacement = Замена
editing-use-null = Использовать NULL (после проверки)
editing-validating = Проверка полного изменения…
editing-reviewed = Проверенное изменение
editing-before = До
editing-after = После
editing-changed-rows = Изменено строк: { $count }
editing-apply = Применить к исходному файлу
editing-review = Проверить изменение
editing-resolving = Разрешение удерживаемых ресурсов сохранения. Не закрывайте окно до получения результата.
editing-applying = Применение проверенного изменения. Не закрывайте окно до получения результата.
editing-cancellation-requested = Запрошена отмена; ожидается фактический результат сохранения.
editing-request-cancellation = Запросить отмену
editing-recovery-owned = Сеанс ещё удерживает ресурсы восстановления. Разрешите его перед продолжением.
editing-recover = Восстановить сохранение
editing-release = Освободить ресурсы сохранения
editing-diagnosis-required = Требуется диагностика; автоматическое восстановление и освобождение недоступны.
editing-target = { $table } · { $column } · строка { $row }
editing-stored-value = Сохранённое значение

editing-mock-rejected = Смоделированная проверка отклонила это значение примера. База данных не открывалась и не изменялась.

editing-mock-failed = Смоделированное сохранение завершилось ошибкой. База данных не открывалась и не изменялась.

editing-mock-diagnosis = Смоделированный результат требует диагностики. В этом режиме нет файла базы данных или журнала.

editing-outcome-saved = Сохранение завершено

editing-outcome-cancelled = Сохранение отменено

editing-outcome-failed = Требуется проверка сохранения

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = зафиксирован
editing-save-known-rolled-back = отменён откатом
editing-save-known-original = исходный образ проверен
editing-save-known-replacement = замещающий образ проверен
editing-save-known-unknown = не установлен
editing-save-completion-applied = применена
editing-save-completion-recovered = восстановлена
editing-save-completion-confirmed = подтверждена
editing-save-completed = Сохранение завершено, снятие блокировки подтверждено.
editing-save-released = Ресурсы освобождены. Файлы восстановления могут остаться; разберитесь с ними перед использованием этого файла. { $details }
editing-save-current = Результат текущей операции: { $outcome }.
editing-save-no-receipt = Квитанция о завершённой операции отсутствует.
editing-save-prior = Последняя завершённая операция: { $completion }; результат: { $outcome }.
editing-save-matched = Совпавшие строки: { $count }.
editing-save-changed = Изменено строк: { $count }.
editing-save-failure = Сохранение завершилось нештатно ({ $reason }). { $details } Не повторяйте автоматически.
editing-save-reason-stale = просматриваемый документ был заменён
editing-save-reason-open = не удалось открыть файл для монопольного редактирования
editing-save-reason-source = файл изменился после просмотра
editing-save-reason-cancelled = была запрошена отмена
editing-save-reason-resource = достигнут лимит операции
editing-save-reason-planning = не удалось проверить изменение
editing-save-reason-intent = не удалось подготовить запись изменения
editing-save-reason-setup = не удалось создать файлы восстановления
editing-save-reason-prepare = не удалось подготовить сохранение
editing-save-reason-commit = фиксация завершилась нештатно
editing-save-reason-recovery = восстановление завершилось нештатно
editing-save-reason-finalization = завершение сохранения прошло нештатно
editing-save-reason-release = снятие блокировки не подтверждено
editing-save-reason-state = сеанс сохранения требует диагностики

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = Логическое значение: true или false
editing-input-byte = Байт: от 0 до 255
editing-input-int16 = 16-битное целое со знаком
editing-input-int32 = 32-битное целое со знаком
editing-input-int64 = 64-битное целое со знаком
editing-input-single = Single: десятичное число Используйте "." как десятичный разделитель.
editing-input-double = Double: десятичное число Используйте "." как десятичный разделитель.
editing-input-currency = Денежная сумма: десятичное значение (четыре знака после запятой) Используйте "." как десятичный разделитель.
editing-input-decimal = Десятичное число Используйте "." как десятичный разделитель.
editing-input-text = Буквальный текст
editing-input-binary = Двоичные данные: пары шестнадцатеричных цифр для каждого байта
editing-input-guid = GUID: канонический идентификатор
editing-input-date = Устаревший DATE: гражданские дата и время ISO (без часового пояса)
editing-input-date-extended = Расширенные дата и время: гражданские дата и время ISO (точность 100 нс)

editing-paste-text = Вставить текст
editing-paste-help = Вставьте буквальный текст в одну выбранную ячейку исходной таблицы и проверьте его. Используйте инвариантный формат ввода поля; форматированные значения и типизированные форматы буфера Access не интерпретируются. Многострочный текст остаётся в одной ячейке.
editing-paste-too-large = Текст в буфере обмена превышает предел 1 МиБ. Ничего не вставлено.
editing-paste-stale = Запрос буфера обмена больше не соответствует выбранной ячейке. Текст отброшен.
editing-paste-multiple = Буфер обмена вернул несколько текстов. Ничего не вставлено.

editing-save-retained-private-journal = После сохранения на диске остаётся закрытый журнал восстановления.

# An actor ended without a known current native outcome.
editing-save-uncertain = Процесс сохранения неожиданно остановился. Файл уже мог измениться. Перезапустите TableCraft и заново откройте базу данных перед дальнейшими изменениями; проверьте оставшийся журнал восстановления. Откат или повторная попытка не подтверждены.
