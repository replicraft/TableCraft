# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = Consulta de atualização
query-action-update-reviewing = Revisando a consulta de atualização…
query-action-table = Tabela: { $table }
query-action-matched = Registros correspondentes: { $count }
query-action-update-changed = Registros com alterações efetivas: { $count }
query-action-update-confirmation = Aplicar estas alterações revisadas ao banco de dados original?
query-action-no-undo = Não é possível desfazer esta operação.
query-action-yes = Sim
query-action-no = Não
query-action-invalid-review = A ação revisada não corresponde mais a este rascunho SQL.
query-action-delete-title = Consulta de exclusão
query-action-delete-reviewing = Revisando a consulta de exclusão…
query-action-delete-deleted = Registros a excluir: { $count }
query-action-delete-confirmation = Excluir permanentemente estes registros revisados do banco de dados original?
query-action-insert-title = Consulta de acréscimo
query-action-insert-reviewing = Revisando a consulta de acréscimo…
query-action-insert-inserted = Registros a acrescentar: { $count }
query-action-insert-confirmation = Acrescentar os registros revisados ao banco de dados original?

## Reviewed SQL schema changes
query-action-schema-title = Consulta de definição de dados
query-action-schema-reviewing = Revisando a consulta de esquema…
query-action-schema-confirmation = Aplicar esta alteração revisada à estrutura do banco de dados?
query-action-schema-create-fields = Campos a criar: { $count }
query-action-schema-drop-records = Registros na tabela a excluir: { $count }
query-action-schema-drop-field = Campo a excluir: { $column }
query-action-schema-create-confirmation = Criar esta tabela revisada no banco de dados original?
query-action-schema-drop-table-confirmation = Excluir permanentemente esta tabela revisada e seus registros do banco de dados original?
query-action-schema-drop-column-confirmation = Excluir permanentemente este campo revisado e seus valores do banco de dados original?
query-action-schema-close-target = Feche a tabela de destino antes de executar esta consulta de esquema.
query-action-schema-reopen-failed = A alteração de esquema foi salva, mas não foi possível reabrir o banco de dados. Não repita a consulta.
query-action-schema-retry-reopen = Tentar reabrir
query-action-schema-retained-results = Os resultados anteriores são mantidos. Reabra o banco de dados salvo para atualizar sua estrutura; não repita a consulta.

query-action-schema-make-table-confirmation = Criar esta tabela e inserir todos os registros selecionados em uma única operação?
query-action-schema-make-table-records = Registros selecionados para inserir: { $count }

query-action-schema-index-name = Índice: { $name }
query-action-schema-index-field = Campo: { $name }
query-action-schema-index-ascending = Ordem: Crescente
query-action-schema-index-descending = Ordem: Decrescente
query-action-schema-index-source-rows = Registros existentes a indexar: { $count }
query-action-schema-index-confirmation = Criar este índice não exclusivo no banco de dados original?

query-action-schema-add-confirmation = Adicionar este campo à tabela original?

query-action-schema-drop-index-name = Índice a remover: { $name }
query-action-schema-drop-index-confirmation = Remover permanentemente este índice do banco de dados original? Os valores dos registros permanecerão inalterados.
