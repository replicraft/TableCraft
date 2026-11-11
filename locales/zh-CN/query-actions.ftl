# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = 更新查询
query-action-update-reviewing = 正在检查更新查询…
query-action-table = 表：{ $table }
query-action-matched = 匹配的记录数：{ $count }
query-action-update-changed = 实际发生更改的记录数：{ $count }
query-action-update-confirmation = 将这些已检查的更改应用到原始数据库吗？
query-action-no-undo = 此操作无法撤销。
query-action-yes = 是
query-action-no = 否
query-action-invalid-review = 已检查的操作与当前 SQL 草稿不再匹配。
query-action-delete-title = 删除查询
query-action-delete-reviewing = 正在检查删除查询…
query-action-delete-deleted = 将删除的记录数：{ $count }
query-action-delete-confirmation = 要从原始数据库中永久删除这些已检查的记录吗？
query-action-insert-title = 追加查询
query-action-insert-reviewing = 正在检查追加查询…
query-action-insert-inserted = 将追加的记录数：{ $count }
query-action-insert-confirmation = 要将已检查的记录追加到原始数据库吗？

## Reviewed SQL schema changes
query-action-schema-title = 数据定义查询
query-action-schema-reviewing = 正在审核架构查询…
query-action-schema-confirmation = 是否应用此已审核的数据库结构更改？
query-action-schema-create-fields = 要创建的字段数：{ $count }
query-action-schema-drop-records = 要删除的表中的记录数：{ $count }
query-action-schema-drop-field = 要删除的字段：{ $column }
query-action-schema-create-confirmation = 是否在原始数据库中创建此已审核的表？
query-action-schema-drop-table-confirmation = 是否从原始数据库中永久删除此已审核的表及其记录？
query-action-schema-drop-column-confirmation = 是否从原始数据库中永久删除此已审核的字段及其值？
query-action-schema-close-target = 运行此架构查询前，请关闭目标表。
query-action-schema-reopen-failed = 架构更改已保存，但无法重新打开数据库。请勿重复执行查询。
query-action-schema-retry-reopen = 重试打开
query-action-schema-retained-results = 保留先前的结果。请重新打开已保存的数据库以刷新其结构；不要重复执行查询。

query-action-schema-make-table-confirmation = 是否创建此表并在一次操作中插入所有选定记录？
query-action-schema-make-table-records = 要插入的选定记录数：{ $count }

query-action-schema-index-name = 索引：{ $name }
query-action-schema-index-field = 字段：{ $name }
query-action-schema-index-ascending = 顺序：升序
query-action-schema-index-descending = 顺序：降序
query-action-schema-index-source-rows = 要编入索引的现有记录数：{ $count }
query-action-schema-index-confirmation = 是否在原始数据库中创建此非唯一索引？

query-action-schema-add-confirmation = 将此字段添加到原始表中？
