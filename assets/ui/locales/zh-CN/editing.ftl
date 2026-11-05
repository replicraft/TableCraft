# TableCraft UI: editing.

editing-title = 离线单元格编辑
editing-exclusive-help = 独占离线编辑。应用前请在 Access 及其他应用程序中关闭此文件。
editing-checking = 正在检查单元格并读取原始值…
editing-original = 原始存储值
editing-replacement = 替换值
editing-use-null = 使用 NULL（仍需验证）
editing-validating = 正在验证完整编辑…
editing-reviewed = 已审阅的更改
editing-before = 更改前
editing-after = 更改后
editing-changed-rows = 更改的行数：{ $count }
editing-apply = 应用到原始文件
editing-review = 审阅更改
editing-resolving = 正在处理保留的保存资源。在结果确定之前请勿关闭此窗口。
editing-applying = 正在应用已审阅的编辑。在结果确定之前请勿关闭此窗口。
editing-cancellation-requested = 已请求取消；正在等待实际保存结果。
editing-request-cancellation = 请求取消
editing-recovery-owned = 保存会话仍持有恢复资源。请先处理会话再继续。
editing-recover = 恢复保存
editing-release = 释放保存资源
editing-diagnosis-required = 需要诊断；无法自动恢复或释放资源。
editing-target = { $table } · { $column } · 第 { $row } 行
editing-stored-value = 存储值

editing-mock-rejected = 模拟验证拒绝了此示例值。未打开或更改任何数据库。

editing-mock-failed = 模拟保存失败。未打开或更改任何数据库。

editing-mock-diagnosis = 模拟结果需要诊断。此预览中不存在数据库文件或日志。

editing-outcome-saved = 保存完成

editing-outcome-cancelled = 保存已取消

editing-outcome-failed = 保存需要检查

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = 已提交
editing-save-known-rolled-back = 已回滚
editing-save-known-original = 已验证原始映像
editing-save-known-replacement = 已验证替换映像
editing-save-known-unknown = 尚未确定
editing-save-completion-applied = 已应用
editing-save-completion-recovered = 已恢复
editing-save-completion-confirmed = 已确认
editing-save-completed = 保存已完成，锁定释放已得到确认。
editing-save-released = 资源已释放。恢复文件可能仍然存在；使用此文件前请先处理。{ $details }
editing-save-current = 当前操作结果：{ $outcome }。
editing-save-no-receipt = 没有已完成操作的回执。
editing-save-prior = 最后完成的操作：{ $completion }；结果：{ $outcome }。
editing-save-matched = 匹配行数：{ $count }。
editing-save-changed = 更改的行数：{ $count }。
editing-save-failure = 保存未正常完成（{ $reason }）。{ $details } 请勿自动重试。
editing-save-reason-stale = 查看的文档已被替换
editing-save-reason-open = 无法以独占编辑方式打开文件
editing-save-reason-source = 文件在查看后发生了更改
editing-save-reason-cancelled = 已请求取消
editing-save-reason-resource = 已达到操作限制
editing-save-reason-planning = 无法验证编辑内容
editing-save-reason-intent = 无法准备更改记录
editing-save-reason-setup = 无法设置恢复文件
editing-save-reason-prepare = 无法准备保存
editing-save-reason-commit = 提交未正常完成
editing-save-reason-recovery = 恢复未正常完成
editing-save-reason-finalization = 保存收尾未正常完成
editing-save-reason-release = 锁定释放未得到确认
editing-save-reason-state = 保存会话需要诊断

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = 布尔值: true 或 false
editing-input-byte = 字节: 0 到 255
editing-input-int16 = 有符号16位整数
editing-input-int32 = 有符号32位整数
editing-input-int64 = 有符号64位整数
editing-input-single = Single: 十进制数请使用“.”作为小数分隔符。
editing-input-double = Double: 十进制数请使用“.”作为小数分隔符。
editing-input-currency = 货币: 十进制金额（四位小数）请使用“.”作为小数分隔符。
editing-input-decimal = 十进制数请使用“.”作为小数分隔符。
editing-input-text = 原样文本
editing-input-binary = 二进制: 每字节使用两位十六进制数
editing-input-guid = GUID: 规范格式标识符
editing-input-date = 旧 DATE: ISO格式本地日期时间（无时区）
editing-input-date-extended = 扩展日期时间: ISO格式本地日期时间（100 ns精度）

editing-paste-text = 粘贴文本
editing-paste-help = 将原样文本粘贴到一个选中的原生单元格中，然后审核。请使用字段的不受区域设置影响的输入格式；不会解析带格式的值或 Access 的类型化剪贴板格式。多行文本保留在一个单元格中。
editing-paste-too-large = 剪贴板文本超过 1 MiB 限制。未粘贴任何内容。
editing-paste-stale = 剪贴板请求已与选中的单元格不匹配。文本已丢弃。
editing-paste-multiple = 剪贴板返回了多个文本内容。未粘贴任何内容。
