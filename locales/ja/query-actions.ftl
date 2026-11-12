# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = 更新クエリ
query-action-update-reviewing = 更新クエリを確認しています…
query-action-table = テーブル: { $table }
query-action-matched = 一致したレコード数: { $count }
query-action-update-changed = 実際に変更されるレコード数: { $count }
query-action-update-confirmation = 確認した変更を元のデータベースに適用しますか？
query-action-no-undo = この操作は元に戻せません。
query-action-yes = はい
query-action-no = いいえ
query-action-invalid-review = 確認した操作が現在の SQL 下書きと一致しなくなりました。
query-action-delete-title = 削除クエリ
query-action-delete-reviewing = 削除クエリを確認しています…
query-action-delete-deleted = 削除するレコード数: { $count }
query-action-delete-confirmation = 確認したレコードを元のデータベースから完全に削除しますか？
query-action-insert-title = 追加クエリ
query-action-insert-reviewing = 追加クエリを確認しています…
query-action-insert-inserted = 追加するレコード数: { $count }
query-action-insert-confirmation = 確認したレコードを元のデータベースに追加しますか？

## Reviewed SQL schema changes
query-action-schema-title = データ定義クエリ
query-action-schema-reviewing = スキーマクエリを確認しています…
query-action-schema-confirmation = 確認済みのデータベース構造の変更を適用しますか？
query-action-schema-create-fields = 作成するフィールド数: { $count }
query-action-schema-drop-records = 削除するテーブル内のレコード数: { $count }
query-action-schema-drop-field = 削除するフィールド: { $column }
query-action-schema-create-confirmation = 確認済みのテーブルを元のデータベースに作成しますか？
query-action-schema-drop-table-confirmation = 確認済みのテーブルとそのレコードを元のデータベースから完全に削除しますか？
query-action-schema-drop-column-confirmation = 確認済みのフィールドとその値を元のデータベースから完全に削除しますか？
query-action-schema-close-target = このスキーマクエリを実行する前に、対象のテーブルを閉じてください。
query-action-schema-reopen-failed = スキーマの変更は保存されましたが、データベースを再度開けませんでした。クエリを繰り返さないでください。
query-action-schema-retry-reopen = 再度開く
query-action-schema-retained-results = 前の結果を保持しています。保存済みデータベースを開き直して構造を更新してください。クエリを再実行しないでください。

query-action-schema-make-table-confirmation = このテーブルを作成し、選択したすべてのレコードを1回の操作で挿入しますか？
query-action-schema-make-table-records = 挿入する選択済みレコード数: { $count }

query-action-schema-index-name = インデックス: { $name }
query-action-schema-index-field = フィールド: { $name }
query-action-schema-index-ascending = 順序: 昇順
query-action-schema-index-descending = 順序: 降順
query-action-schema-index-source-rows = インデックスを作成する既存レコード数: { $count }
query-action-schema-index-confirmation = 元のデータベースにこの重複を許可するインデックスを作成しますか？

query-action-schema-add-confirmation = 元のテーブルにこのフィールドを追加しますか？

query-action-schema-drop-index-name = 削除するインデックス: { $name }
query-action-schema-drop-index-confirmation = 元のデータベースからこのインデックスを完全に削除しますか？ レコードの値は変更されません。
query-action-schema-index-unique = NULL 以外の値は一意である必要があります。NULL は引き続き使用できます。
query-action-schema-index-unique-confirmation = 元のデータベースにこの一意のインデックスを作成しますか？
