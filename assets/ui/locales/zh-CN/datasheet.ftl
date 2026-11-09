# TableCraft UI: datasheet.

datasheet-column-count = 列数：{ $count }
datasheet-previous-page = ‹ 上一页
datasheet-next-page = 下一页 ›
datasheet-row-range = 第 { $first }–{ $last } 行
datasheet-no-rows = 没有行。
datasheet-field-name = 字段名称：{ $name }
datasheet-select-record = 选择记录 { $number }
datasheet-copy-record = 复制显示的记录
datasheet-record-selected-help = 已选择记录 { $number }。复制所有显示字段及集合摘要，不复制子项数据。
datasheet-select-record-help = 选择记录 { $number }。复制所有显示字段及集合摘要，不复制子项数据。
datasheet-view-contents = 查看内容
datasheet-edit-cell = 编辑单元格…
datasheet-cell-context = { $column }，第 { $row } 行：{ $text }
datasheet-cell-details = { $column }，第 { $row } 行：{ $text }；{ $context }
datasheet-copy-cell-help = 复制操作会复制单元格的完整值。
datasheet-copy-incomplete = 所选记录不包含所有显示的字段。
datasheet-copy-limit = 显示的记录超过剪贴板的 1 MiB 限制。
datasheet-copy-allocation = 无法分配剪贴板缓冲区。
datasheet-copy-encoding = 无法将显示的记录编码到剪贴板。
datasheet-record = 记录：
datasheet-first = 第一条
datasheet-previous = 上一条
datasheet-next = 下一条
datasheet-go-record = 转到记录
datasheet-record-input-help = 请输入 1 到 4294967296 之间的整数记录编号。
datasheet-wrong-source = 返回的页面属于另一个数据源。
datasheet-wrong-offset = 返回页面的起始记录位置不正确。
datasheet-missing-column = 返回的页面不包含所请求的列。
datasheet-missing-record = 快照中的该位置没有记录。
datasheet-incomplete-page = 返回的页面不包含所请求的记录。
datasheet-badge-stored = 已存储
datasheet-badge-null = NULL
datasheet-badge-list = 列表
datasheet-badge-encoded = 已编码
datasheet-badge-history = 历史
datasheet-null-context = NULL 值
datasheet-stored-context = 存储的计算结果；未计算公式；时效性未知
datasheet-stored-null-context = 存储的 NULL 计算结果；未计算公式；时效性未知
datasheet-control-uninspected = 尚未检查字段显示设置；显示存储值
datasheet-control-absent = 未声明显示控件；显示存储值
datasheet-control-unsupported = 不支持此字段显示设置；显示存储值
datasheet-control-textbox = 此字段使用文本框
datasheet-control-lookup = 尚不支持查阅显示；显示存储值
datasheet-context-pair = { $base }；{ $presentation }

datasheet-go = 转到

datasheet-null-value = NULL
datasheet-close-table = 关闭表
datasheet-column-menu = 列菜单 { $column }

datasheet-last = 最后一条

datasheet-value-preview-truncated = 预览已缩短。复制会保留完整的显示值。

datasheet-row-identity-invalid = 记录标识与此页不匹配。
datasheet-filter-page-unavailable = 此处仅可访问已加载的筛选记录。

datasheet-click-to-add = 单击以添加

datasheet-add-field-unavailable = 暂不支持添加字段。

datasheet-record-position = 记录编号

datasheet-record-of = 共 { $count } 条

datasheet-count-unknown = ?

datasheet-search-records = 搜索记录

datasheet-no-filter = 无筛选

datasheet-filtered = 已筛选
datasheet-page-tools = 页面工具

# Saved-table Number field review and checked reopen.
datasheet-number-inspecting = 正在检查能否添加数字字段…
datasheet-number-reviewing = 正在检查新的数字字段…
datasheet-number-reopening = 正在重新打开已保存的表…
datasheet-number-retry-inspection = 重新检查字段添加条件

# Saved Number field close confirmation
datasheet-number-close-title = 是否保存字段更改？
datasheet-number-close-question = 是否将新字段“{ $column }”保存到表“{ $table }”？
datasheet-number-close-stale = 字段名称或表已更改。请关闭此提示后重试。
datasheet-number-close-unavailable = 当前无法保存此字段。
datasheet-number-close-save = 保存
datasheet-number-close-discard = 不保存
datasheet-number-close-cancel = 取消

# Interrupted readonly field eligibility observation.
datasheet-number-inspection-interrupted = 字段检查已中断。请重试以再次检查此表。
