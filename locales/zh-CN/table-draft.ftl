# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = 未保存的表已不可用。
table-draft-new-record = (新建)
table-draft-short-text = 短文本 (255)
table-draft-field-name = 字段名称
table-draft-invalid-name = 请选择有效且唯一的字段名称。
table-draft-empty-note = 请保存此表以继续。
table-draft-save = 保存
table-draft-save-unavailable = 当前无法保存此表。请保持草稿打开或放弃更改。
table-draft-close-title = 未保存的表
table-draft-close-question = 保存对未保存的表所做的更改？
table-draft-discard = 放弃
table-draft-cancel = 取消

table-draft-save-as = 另存为
table-draft-table-name = 表名称
table-draft-invalid-table-name = 请选择不超过 64 个字符的有效表名称。
table-draft-ok = 确定
table-draft-save-cancelled = 已取消保存。表草稿未更改。
table-draft-save-failed = 无法保存表。
table-draft-save-retained = 保存结果需要恢复或验证。请保持此窗口打开。
table-draft-save-working = 正在保存表…
table-draft-save-cancelling = 正在取消保存…
table-draft-save-stale = 此保存操作已不再属于当前表草稿。
table-draft-save-blocked = 保存结果未解决，仍禁止保存。请重新打开数据库检查，或放弃此草稿。
table-draft-save-reopen-failed = 表已保存，但未能完成重新打开。再次保存此草稿前，请重新打开数据库。

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = 大数字
table-draft-currency = 货币
table-draft-date-time = 日期和时间
table-draft-yes-no = 是/否
table-draft-long-text = 长文本
table-draft-lookup-relationship = 查阅和关系
table-draft-rich-text = 格式文本
table-draft-attachment = 附件
table-draft-hyperlink = 超链接
table-draft-calculated-field = 计算字段
table-draft-more-fields = 更多字段
table-draft-record-field = 新记录 { $field }
table-draft-record-schema-saved = 表已保存；此记录尚未保存。

table-record-preparing = 表已保存。正在准备新记录…
table-record-retry = 重试新记录
table-record-discard = 放弃新记录
table-record-invalid = 表已保存，但无法准备新记录。输入内容已保留。

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = { $field } 的字段选项
table-draft-rename-field = 重命名字段
table-draft-delete-field = 删除字段
table-draft-delete-field-title = 删除未保存的字段
table-draft-delete-field-question = 要删除未保存的字段“{ $field }”吗？
table-draft-delete-field-warning = 这将从此未保存的表中移除该字段及其待保存的记录输入。数据库尚未更改。
table-draft-delete-field-confirm = 删除字段

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = 编辑未保存表的名称、类型和受支持的字段属性。其他属性暂不可用。
table-draft-design-name = { $field } 的字段名称
table-draft-design-type = { $field } 的数据类型
table-draft-design-input-help = 更改类型时，待保存的记录输入将保持原样。在审阅和应用之前，输入必须符合所选类型。
table-draft-design-protected = 初始自动编号主键无法更改。
table-draft-design-add-field = 添加字段

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = 未设置
table-draft-property-no = 否
table-draft-property-yes = 是
table-draft-property-type-default = 类型默认值
table-draft-property-absent = 无默认值
table-draft-property-null = NULL
table-draft-property-literal = 字面量
table-draft-property-literal-input = 默认值字面量
table-draft-property-invalid-size = 请输入 1 到 255 之间的整数。
table-draft-property-input-limit = 属性输入超出限制。已保留原来的文本。
table-draft-property-invalid = 保存前请更正无效的字段属性。
table-draft-property-help = 仅限常量：文本无需 SQL 引号，数字使用小数点，布尔值使用 true/false，日期和时间使用 ISO 格式。不会计算公式。输入上限为 4096 个 UTF-8 字节；保存的默认值必须在 255 个 UTF-16 单位以内，包括添加和转义的引号。

table-draft-number = 数字
table-draft-number-byte = 字节
table-draft-number-integer = 整数
table-draft-number-single = 单精度
table-draft-number-double = 双精度
table-draft-number-decimal = 小数
table-draft-number-replication-id = 复制 ID
table-draft-number-precision = 精度
table-draft-number-scale = 小数位数
table-draft-number-invalid-decimal = 精度必须为 1–28，小数位数必须为 0 到精度。保存前请更正这两个值。
table-draft-number-guid-default = 复制 ID 尚不支持字面量默认值。请选择其他模式；已输入的文本会保留。

table-draft-design-description = { $field } 的说明
table-draft-description-invalid = 字段说明过长。

table-draft-no-key-title = 没有主键
table-draft-no-key-question = 尚未定义主键。是否创建自动编号主键？
table-draft-no-key-failed = 无法添加主键。请保留当前设计或删除一个字段。

table-draft-design-new-name = 新字段名称
table-draft-design-new-type = 新字段数据类型

table-draft-field-limit = 表最多支持 255 个字段，包括主键。
