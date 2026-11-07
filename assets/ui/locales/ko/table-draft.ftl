# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = 저장하지 않은 테이블을 더 이상 사용할 수 없습니다.
table-draft-new-record = (새 항목)
table-draft-short-text = 짧은 텍스트 (255)
table-draft-long-integer = 숫자 (긴 정수)
table-draft-field-name = 필드 이름
table-draft-invalid-name = 올바른 고유 필드 이름을 선택하세요.
table-draft-empty-note = 저장되지 않은 테이블입니다. 레코드 값은 테이블을 저장한 후 검토하고 적용할 때까지 초안으로 유지됩니다.
table-draft-save = 저장
table-draft-save-unavailable = 현재 이 테이블을 저장할 수 없습니다. 초안을 열어 두거나 변경 내용을 버리세요.
table-draft-close-title = 저장하지 않은 테이블
table-draft-close-question = 저장하지 않은 테이블의 변경 내용을 저장할까요?
table-draft-discard = 버리기
table-draft-cancel = 취소

table-draft-save-as = 다른 이름으로 저장
table-draft-table-name = 테이블 이름
table-draft-invalid-table-name = 64자 이하의 올바른 테이블 이름을 선택하세요.
table-draft-ok = 확인
table-draft-save-cancelled = 저장을 취소했습니다. 테이블 초안은 변경되지 않았습니다.
table-draft-save-failed = 테이블을 저장하지 못했습니다.
table-draft-save-retained = 저장 결과를 복구하거나 확인해야 합니다. 이 창을 열어 두세요.
table-draft-save-working = 테이블 저장 중…
table-draft-save-cancelling = 저장 취소 중…
table-draft-save-stale = 저장이 더 이상 이 테이블 초안에 해당하지 않습니다.
table-draft-save-blocked = 저장 결과가 해결되지 않아 저장이 차단되었습니다. 데이터베이스를 다시 열어 확인하거나 이 초안을 버리세요.
table-draft-save-reopen-failed = 테이블은 저장되었지만 다시 열기를 완료하지 못했습니다. 이 초안을 다시 저장하기 전에 데이터베이스를 다시 여세요.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = 큰 숫자
table-draft-currency = 통화
table-draft-date-time = 날짜 및 시간
table-draft-yes-no = 예/아니요
table-draft-long-text = 긴 텍스트
table-draft-lookup-relationship = 조회 및 관계
table-draft-rich-text = 서식 있는 텍스트
table-draft-attachment = 첨부 파일
table-draft-hyperlink = 하이퍼링크
table-draft-calculated-field = 계산 필드
table-draft-more-fields = 기타 필드
table-draft-record-field = 새 레코드 { $field }
table-draft-record-schema-saved = 테이블이 저장되었습니다. 이 레코드는 아직 저장되지 않았습니다.

table-record-preparing = 테이블이 저장되었습니다. 새 레코드를 준비하는 중…
table-record-retry = 새 레코드 다시 시도
table-record-discard = 새 레코드 버리기
table-record-invalid = 테이블은 저장되었지만 새 레코드를 준비하지 못했습니다. 입력 내용은 유지됩니다.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = { $field } 필드 옵션
table-draft-rename-field = 필드 이름 바꾸기
table-draft-delete-field = 필드 삭제
table-draft-delete-field-title = 저장하지 않은 필드 삭제
table-draft-delete-field-question = 저장하지 않은 필드 “{ $field }”을(를) 삭제하시겠습니까?
table-draft-delete-field-warning = 이 저장하지 않은 테이블에서 필드와 해당 레코드 입력이 제거됩니다. 데이터베이스는 변경되지 않았습니다.
table-draft-delete-field-confirm = 필드 삭제

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = 저장하지 않은 테이블의 이름, 형식 및 지원되는 필드 속성을 편집합니다. 다른 속성은 아직 사용할 수 없습니다.
table-draft-design-name = { $field }의 필드 이름
table-draft-design-type = { $field }의 데이터 형식
table-draft-design-input-help = 형식을 변경해도 보류 중인 레코드 입력은 그대로 유지됩니다. 검토하고 적용하기 전에 선택한 형식에 맞는지 확인해야 합니다.
table-draft-design-protected = 초기 일련 번호 기본 키는 변경할 수 없습니다.
table-draft-design-add-field = 필드 추가

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = 설정 안 됨
table-draft-property-no = 아니요
table-draft-property-yes = 예
table-draft-property-type-default = 형식 기본값
table-draft-property-absent = 기본값 없음
table-draft-property-null = NULL
table-draft-property-literal = 리터럴
table-draft-property-literal-input = 기본값 리터럴
table-draft-property-invalid-size = 1부터 255까지의 정수를 입력하세요.
table-draft-property-input-limit = 속성 입력이 제한을 초과했습니다. 이전 텍스트는 유지됩니다.
table-draft-property-invalid = 저장하기 전에 잘못된 필드 속성을 수정하세요.
table-draft-property-help = 상수만 입력하세요. 텍스트에는 SQL 따옴표를 붙이지 않고, 숫자에는 소수점으로 마침표를 사용하세요. 논리값은 true/false, 날짜와 시간은 ISO 형식으로 입력하세요. 수식은 계산하지 않습니다. 입력 한도는 UTF-8 4096바이트이며, 저장할 기본값은 추가되거나 이스케이프된 따옴표를 포함해 UTF-16 255단위 이내여야 합니다.
