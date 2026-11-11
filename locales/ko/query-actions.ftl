# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = 업데이트 쿼리
query-action-update-reviewing = 업데이트 쿼리를 검토하는 중…
query-action-table = 테이블: { $table }
query-action-matched = 일치하는 레코드: { $count }
query-action-update-changed = 실제로 변경되는 레코드: { $count }
query-action-update-confirmation = 검토한 변경 내용을 원본 데이터베이스에 적용하시겠습니까?
query-action-no-undo = 이 작업은 실행 취소할 수 없습니다.
query-action-yes = 예
query-action-no = 아니요
query-action-invalid-review = 검토한 작업이 현재 SQL 초안과 더 이상 일치하지 않습니다.
query-action-delete-title = 삭제 쿼리
query-action-delete-reviewing = 삭제 쿼리를 검토하는 중…
query-action-delete-deleted = 삭제할 레코드: { $count }
query-action-delete-confirmation = 검토한 레코드를 원본 데이터베이스에서 영구적으로 삭제하시겠습니까?
query-action-insert-title = 추가 쿼리
query-action-insert-reviewing = 추가 쿼리를 검토하는 중…
query-action-insert-inserted = 추가할 레코드: { $count }
query-action-insert-confirmation = 검토한 레코드를 원본 데이터베이스에 추가하시겠습니까?

## Reviewed SQL schema changes
query-action-schema-title = 데이터 정의 쿼리
query-action-schema-reviewing = 스키마 쿼리를 검토하는 중…
query-action-schema-confirmation = 검토한 데이터베이스 구조 변경을 적용하시겠습니까?
query-action-schema-create-fields = 만들 필드 수: { $count }
query-action-schema-drop-records = 삭제할 테이블의 레코드 수: { $count }
query-action-schema-drop-field = 삭제할 필드: { $column }
query-action-schema-create-confirmation = 검토한 테이블을 원본 데이터베이스에 만드시겠습니까?
query-action-schema-drop-table-confirmation = 검토한 테이블과 해당 레코드를 원본 데이터베이스에서 영구적으로 삭제하시겠습니까?
query-action-schema-drop-column-confirmation = 검토한 필드와 해당 값을 원본 데이터베이스에서 영구적으로 삭제하시겠습니까?
query-action-schema-close-target = 이 스키마 쿼리를 실행하기 전에 대상 테이블을 닫으세요.
query-action-schema-reopen-failed = 스키마 변경은 저장되었지만 데이터베이스를 다시 열 수 없습니다. 쿼리를 반복하지 마세요.
query-action-schema-retry-reopen = 다시 열기
query-action-schema-retained-results = 이전 결과가 유지됩니다. 저장된 데이터베이스를 다시 열어 구조를 새로 고치세요. 쿼리를 다시 실행하지 마세요.

query-action-schema-make-table-confirmation = 이 테이블을 만들고 선택한 모든 레코드를 하나의 작업으로 삽입하시겠습니까?
query-action-schema-make-table-records = 삽입할 선택된 레코드: { $count }

query-action-schema-index-name = 인덱스: { $name }
query-action-schema-index-field = 필드: { $name }
query-action-schema-index-ascending = 순서: 오름차순
query-action-schema-index-descending = 순서: 내림차순
query-action-schema-index-source-rows = 인덱싱할 기존 레코드: { $count }
query-action-schema-index-confirmation = 원본 데이터베이스에 이 중복 허용 인덱스를 만드시겠습니까?
