# TableCraft UI: editing.

editing-title = オフラインでセルを編集
editing-exclusive-help = 排他的なオフライン編集です。適用する前に、Access や他のアプリでこのファイルを閉じてください。
editing-checking = セルを確認し、元の値を読み取り中…
editing-original = 元の保存値
editing-replacement = 置換する値
editing-use-null = NULL を使用（検証が必要）
editing-validating = 編集全体を検証中…
editing-reviewed = 確認済みの変更
editing-before = 変更前
editing-after = 変更後
editing-changed-rows = 変更する行数: { $count }
editing-apply = 元のファイルに適用
editing-review = 変更を確認
editing-resolving = 保持されている保存リソースを処理中です。結果が確定するまでこのウィンドウを開いたままにしてください。
editing-applying = 確認済みの編集を適用中です。結果が確定するまでこのウィンドウを開いたままにしてください。
editing-cancellation-requested = キャンセルを要求しました。実際の保存結果を待っています。
editing-request-cancellation = キャンセルを要求
editing-recovery-owned = 保存セッションは復旧リソースを保持しています。続行する前に解決してください。
editing-recover = 保存の復旧
editing-release = 保存リソースを解放
editing-diagnosis-required = 診断が必要です。自動復旧と自動解放は利用できません。
editing-target = { $table } · { $column } · 行 { $row }
editing-stored-value = 保存値

editing-mock-rejected = 検証のシミュレーションでこのサンプル値が拒否されました。データベースは開かれておらず、変更もされていません。

editing-mock-failed = 保存のシミュレーションに失敗しました。データベースは開かれておらず、変更もされていません。

editing-mock-diagnosis = シミュレーションの結果には診断が必要です。このプレビューにはデータベース ファイルもジャーナルもありません。

editing-outcome-saved = 保存完了

editing-outcome-cancelled = 保存のキャンセル

editing-outcome-failed = 保存状態の確認が必要です

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = コミット済み
editing-save-known-rolled-back = ロールバック済み
editing-save-known-original = 元のイメージを検証済み
editing-save-known-replacement = 置換イメージを検証済み
editing-save-known-unknown = 未確定
editing-save-completion-applied = 適用
editing-save-completion-recovered = 復旧
editing-save-completion-confirmed = 確認
editing-save-completed = 保存が完了し、ロックの解放が確認されました。
editing-save-released = リソースを解放しました。復旧ファイルが残っている可能性があります。このファイルを使用する前に対処してください。{ $details }
editing-save-current = 現在の操作の結果: { $outcome }。
editing-save-no-receipt = 完了した操作の受領記録はありません。
editing-save-prior = 最後に完了した操作: { $completion }、結果: { $outcome }。
editing-save-matched = 一致した行数: { $count }。
editing-save-changed = 変更する行数: { $count }。
editing-save-failure = 保存が正常に完了しませんでした（{ $reason }）。{ $details } 自動的に再試行しないでください。
editing-save-reason-stale = 表示中のドキュメントが置き換えられました
editing-save-reason-open = 排他編集用にファイルを開けませんでした
editing-save-reason-source = 表示後にファイルが変更されました
editing-save-reason-cancelled = キャンセルが要求されました
editing-save-reason-resource = 操作の制限に達しました
editing-save-reason-planning = 編集内容を検証できませんでした
editing-save-reason-intent = 変更記録を準備できませんでした
editing-save-reason-setup = 復旧ファイルを設定できませんでした
editing-save-reason-prepare = 保存を準備できませんでした
editing-save-reason-commit = コミットが正常に完了しませんでした
editing-save-reason-recovery = 復旧が正常に完了しませんでした
editing-save-reason-finalization = 保存の終了処理が正常に完了しませんでした
editing-save-reason-release = ロックの解放が確認されませんでした
editing-save-reason-state = 保存セッションの診断が必要です

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = 真偽値: true または false
editing-input-byte = バイト: 0～255
editing-input-int16 = 符号付き16ビット整数
editing-input-int32 = 符号付き32ビット整数
editing-input-int64 = 符号付き64ビット整数
editing-input-single = Single: 小数小数点には「.」を使用してください。
editing-input-double = Double: 小数小数点には「.」を使用してください。
editing-input-currency = 通貨: 小数の金額（小数点以下4桁）小数点には「.」を使用してください。
editing-input-decimal = 10進数小数点には「.」を使用してください。
editing-input-text = 入力したままの文字列
editing-input-binary = バイナリ: 各バイトを2桁の16進数で入力
editing-input-guid = GUID: 正規形式の識別子
editing-input-date = 従来の DATE: ISO形式のローカル日時（タイムゾーンなし）
editing-input-date-extended = 拡張日時: ISO形式のローカル日時（100 ns精度）
