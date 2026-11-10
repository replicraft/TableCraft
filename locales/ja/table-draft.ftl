# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = 未保存のテーブルは使用できなくなりました。
table-draft-new-record = (新規)
table-draft-short-text = 短いテキスト (255)
table-draft-field-name = フィールド名
table-draft-invalid-name = 有効で一意のフィールド名を指定してください。
table-draft-empty-note = 続行するには、このテーブルを保存してください。
table-draft-save = 保存
table-draft-save-unavailable = 現在、このテーブルは保存できません。下書きを開いたままにするか、変更を破棄してください。
table-draft-close-title = 未保存のテーブル
table-draft-close-question = 未保存のテーブルの変更を保存しますか？
table-draft-discard = 破棄
table-draft-cancel = キャンセル

table-draft-save-as = 名前を付けて保存
table-draft-table-name = テーブル名
table-draft-invalid-table-name = 64文字以内の有効なテーブル名を指定してください。
table-draft-ok = OK
table-draft-save-cancelled = 保存をキャンセルしました。テーブルの下書きは変更されていません。
table-draft-save-failed = テーブルを保存できませんでした。
table-draft-save-retained = 保存結果の復旧または確認が必要です。このウィンドウを開いたままにしてください。
table-draft-save-working = テーブルを保存中…
table-draft-save-cancelling = 保存をキャンセル中…
table-draft-save-stale = この保存は現在のテーブルの下書きに対応していません。
table-draft-save-blocked = 保存結果が未解決のため、保存はブロックされています。データベースを開き直して確認するか、この下書きを破棄してください。
table-draft-save-reopen-failed = テーブルは保存されましたが、開き直すことができませんでした。この下書きを再度保存する前に、データベースを開き直してください。

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = 大きい数値
table-draft-currency = 通貨
table-draft-date-time = 日付と時刻
table-draft-yes-no = Yes/No
table-draft-long-text = 長いテキスト
table-draft-lookup-relationship = ルックアップとリレーションシップ
table-draft-rich-text = リッチ テキスト
table-draft-attachment = 添付ファイル
table-draft-hyperlink = ハイパーリンク
table-draft-calculated-field = 集計フィールド
table-draft-more-fields = その他のフィールド
table-draft-record-field = 新しいレコード { $field }
table-draft-record-schema-saved = テーブルは保存されました。このレコードはまだ保存されていません。

table-record-preparing = テーブルを保存しました。新しいレコードを準備しています…
table-record-retry = 新しいレコードを再試行
table-record-discard = 新しいレコードを破棄
table-record-invalid = テーブルを保存しましたが、新しいレコードを準備できませんでした。入力内容は保持されています。

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = { $field } のフィールドオプション
table-draft-rename-field = フィールド名の変更
table-draft-delete-field = フィールドの削除
table-draft-delete-field-title = 未保存のフィールドを削除
table-draft-delete-field-question = 未保存のフィールド「{ $field }」を削除しますか？
table-draft-delete-field-warning = この未保存のテーブルから、フィールドとそのレコード入力が削除されます。データベースは変更されていません。
table-draft-delete-field-confirm = フィールドを削除

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = 未保存のテーブルの名前、型、および対応するフィールドプロパティを編集できます。その他のプロパティはまだ利用できません。
table-draft-design-name = { $field } のフィールド名
table-draft-design-type = { $field } のデータ型
table-draft-design-input-help = 型を変更しても、保留中のレコード入力はそのまま保持されます。確認して適用する前に、選択した型に適合させてください。
table-draft-design-protected = 最初のオートナンバー型の主キーは変更できません。
table-draft-design-add-field = フィールドを追加

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = 未設定
table-draft-property-no = いいえ
table-draft-property-yes = はい
table-draft-property-type-default = 型の既定値
table-draft-property-absent = 既定値なし
table-draft-property-null = NULL
table-draft-property-literal = リテラル
table-draft-property-literal-input = 既定値のリテラル
table-draft-property-invalid-size = 1 から 255 までの整数を入力してください。
table-draft-property-input-limit = プロパティの入力が上限を超えています。元のテキストは保持されます。
table-draft-property-invalid = 保存する前に、無効なフィールドプロパティを修正してください。
table-draft-property-help = 定数のみです。テキストは SQL の引用符なしで、数値は小数点にピリオドを使い、真偽値は true/false、日時は ISO 形式で入力します。数式は評価しません。入力は UTF-8 で4096バイト以内、保存する既定値は追加・エスケープされる引用符も含めて UTF-16 で255単位以内です。

table-draft-number = 数値
table-draft-number-byte = バイト型
table-draft-number-integer = 整数型
table-draft-number-single = 単精度浮動小数点型
table-draft-number-double = 倍精度浮動小数点型
table-draft-number-decimal = 十進型
table-draft-number-replication-id = レプリケーション ID
table-draft-number-precision = 精度
table-draft-number-scale = 小数点以下桁数
table-draft-number-invalid-decimal = 精度は 1～28、小数点以下桁数は 0～精度の範囲で指定します。保存する前に両方の値を修正してください。
table-draft-number-guid-default = レプリケーション ID のリテラル既定値はまだ利用できません。別のモードを選択してください。入力したテキストは保持されます。

table-draft-design-description = { $field } の説明
table-draft-description-invalid = フィールドの説明が長すぎます。

table-draft-no-key-title = 主キーがありません
table-draft-no-key-question = 主キーが定義されていません。オートナンバー型の主キーを作成しますか？
table-draft-no-key-failed = 主キーを追加できませんでした。現在の設計を維持するか、フィールドを削除してください。

table-draft-design-new-name = 新しいフィールド名
table-draft-design-new-type = 新しいフィールドのデータ型

table-draft-field-limit = Access のテーブルは主キーを含めて最大255個のフィールドをサポートします。
