# TableCraft UI: editing.

editing-title = 오프라인 셀 편집
editing-exclusive-help = 배타적인 오프라인 편집입니다. 적용하기 전에 Access 및 다른 응용 프로그램에서 이 파일을 닫으세요.
editing-checking = 셀을 확인하고 원래 값을 읽는 중…
editing-original = 원래 저장된 값
editing-replacement = 바꿀 값
editing-use-null = NULL 사용(유효성 검사 필요)
editing-validating = 전체 편집 내용을 검증하는 중…
editing-reviewed = 검토된 변경 내용
editing-before = 변경 전
editing-after = 변경 후
editing-changed-rows = 변경된 행 수: { $count }
editing-apply = 원본 파일에 적용
editing-review = 변경 내용 검토
editing-resolving = 유지된 저장 리소스를 처리하는 중입니다. 결과가 확인될 때까지 이 창을 닫지 마세요.
editing-applying = 검토된 편집 내용을 적용하는 중입니다. 결과가 확인될 때까지 이 창을 닫지 마세요.
editing-cancellation-requested = 취소를 요청했습니다. 실제 저장 결과를 기다리는 중입니다.
editing-request-cancellation = 취소 요청
editing-recovery-owned = 저장 세션이 아직 복구 리소스를 유지하고 있습니다. 계속하기 전에 해결하세요.
editing-recover = 저장 복구
editing-release = 저장 리소스 해제
editing-diagnosis-required = 진단이 필요합니다. 자동 복구 및 해제를 사용할 수 없습니다.
editing-target = { $table } · { $column } · 행 { $row }
editing-stored-value = 저장된 값

editing-mock-rejected = 검증 시뮬레이션에서 이 샘플 값을 거부했습니다. 데이터베이스를 열거나 변경하지 않았습니다.

editing-mock-failed = 저장 시뮬레이션이 실패했습니다. 데이터베이스를 열거나 변경하지 않았습니다.

editing-mock-diagnosis = 시뮬레이션 결과에 진단이 필요합니다. 이 미리 보기에는 데이터베이스 파일이나 저널이 없습니다.

editing-outcome-saved = 저장 완료

editing-outcome-cancelled = 저장 취소됨

editing-outcome-failed = 저장 상태 확인 필요

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = 커밋됨
editing-save-known-rolled-back = 롤백됨
editing-save-known-original = 원본 이미지 확인됨
editing-save-known-replacement = 대체 이미지 확인됨
editing-save-known-unknown = 확정되지 않음
editing-save-completion-applied = 적용됨
editing-save-completion-recovered = 복구됨
editing-save-completion-confirmed = 확인됨
editing-save-completed = 저장이 완료되었으며 잠금 해제가 확인되었습니다.
editing-save-released = 리소스가 해제되었습니다. 복구 파일이 남아 있을 수 있습니다. 이 파일을 사용하기 전에 해결하세요. { $details }
editing-save-current = 현재 작업 결과: { $outcome }.
editing-save-no-receipt = 완료된 작업의 확인 기록이 없습니다.
editing-save-prior = 마지막으로 완료된 작업: { $completion }; 결과: { $outcome }.
editing-save-matched = 일치한 행: { $count }.
editing-save-changed = 변경된 행 수: { $count }.
editing-save-failure = 저장이 정상적으로 완료되지 않았습니다({ $reason }). { $details } 자동으로 다시 시도하지 마세요.
editing-save-reason-stale = 표시 중인 문서가 교체되었습니다
editing-save-reason-open = 단독 편집을 위해 파일을 열 수 없었습니다
editing-save-reason-source = 표시한 후 파일이 변경되었습니다
editing-save-reason-cancelled = 취소가 요청되었습니다
editing-save-reason-resource = 작업 한도에 도달했습니다
editing-save-reason-planning = 편집 내용을 검증할 수 없었습니다
editing-save-reason-intent = 변경 기록을 준비할 수 없었습니다
editing-save-reason-setup = 복구 파일을 설정할 수 없었습니다
editing-save-reason-prepare = 저장을 준비할 수 없었습니다
editing-save-reason-commit = 커밋이 정상적으로 완료되지 않았습니다
editing-save-reason-recovery = 복구가 정상적으로 완료되지 않았습니다
editing-save-reason-finalization = 저장 마무리 처리가 정상적으로 완료되지 않았습니다
editing-save-reason-release = 잠금 해제가 확인되지 않았습니다
editing-save-reason-state = 저장 세션의 진단이 필요합니다
