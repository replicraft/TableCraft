# Saved-field deletion. These prompts are TableCraft adaptations; no retained Access dialog capture exists.

## Confirmation

field-drop-title = 删除字段
field-drop-confirm = 是否要删除字段“{ $column }”？
field-drop-data-loss = 保存表时，将永久删除该字段及其数据。
field-drop-yes = 是
field-drop-no = 否

## Source inspection

field-drop-inspecting = 正在检查是否可以删除此字段…
field-drop-source-unavailable = 当前编辑配置不支持删除此字段。
field-drop-inspection-interrupted = 字段检查已中断。
field-drop-retry-inspection = 重新检查

## Pending deletion

field-drop-pending = 待删除：{ $column }
field-drop-pending-help = 保存表以删除此字段。只有验证已保存的表后，当前视图才会改变。
field-drop-design-help = 选择字段并选择“删除行”以将其标记为待删除，然后保存表以应用更改。

## Save and reopen

field-drop-reviewing = 正在检查请求的字段删除操作…
field-drop-reopening = 正在验证已保存的表，然后将其重新打开…
field-drop-return = 返回表
field-drop-warning-saved = 字段删除已保存，但无法重新打开表。请在编辑前重新打开数据库。

## Closing and retained status

field-drop-close-title = 保存表
field-drop-close-question = 是否保存表“{ $table }”中字段“{ $column }”的待删除操作？
field-drop-close-save = 保存
field-drop-close-discard = 放弃
field-drop-close-cancel = 取消
field-drop-close-stale = 此确认已不再与当前表匹配。
field-drop-close-unavailable = 当前状态下无法保存删除操作。
field-drop-close-warning = 是否关闭此删除状态显示？已保存的删除操作和所有清理通知仍会保留记录。
field-drop-close-editor = 关闭删除状态显示
