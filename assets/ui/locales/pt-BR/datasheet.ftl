# TableCraft UI: datasheet.

datasheet-column-count = Colunas: { $count }
datasheet-previous-page = ‹ Página anterior
datasheet-next-page = Próxima página ›
datasheet-row-range = Linhas { $first }–{ $last }
datasheet-no-rows = Nenhuma linha.
datasheet-field-name = Nome do campo: { $name }
datasheet-select-record = Selecionar registro { $number }
datasheet-copy-record = Copiar registro exibido
datasheet-record-selected-help = O registro { $number } está selecionado. A cópia inclui todos os campos exibidos e os resumos de coleções, sem os dados filhos.
datasheet-select-record-help = Selecione o registro { $number }. A cópia inclui todos os campos exibidos e os resumos de coleções, sem os dados filhos.
datasheet-view-contents = Exibir conteúdo
datasheet-edit-cell = Editar célula…
datasheet-cell-context = { $column }, linha { $row }: { $text }
datasheet-cell-details = { $column }, linha { $row }: { $text }; { $context }
datasheet-copy-cell-help = Copiar inclui o valor completo da célula.
datasheet-copy-incomplete = O registro selecionado não contém todos os campos exibidos.
datasheet-copy-limit = O registro exibido excede o limite de 1 MiB da área de transferência.
datasheet-copy-allocation = Não foi possível alocar o buffer da área de transferência.
datasheet-copy-encoding = Não foi possível codificar o registro exibido para a área de transferência.
datasheet-record = Registro:
datasheet-first = Primeiro
datasheet-previous = Anterior
datasheet-next = Próximo
datasheet-go-record = Ir para registro
datasheet-record-input-help = Digite um número inteiro de registro entre 1 e 4294967296.
datasheet-wrong-source = A página recebida pertence a outra origem.
datasheet-wrong-offset = A página recebida começa em outra posição de registro.
datasheet-missing-column = A coluna solicitada não está na página recebida.
datasheet-missing-record = Não há registro nessa posição do instantâneo.
datasheet-incomplete-page = A página recebida não contém o registro solicitado.
datasheet-badge-stored = armazenado
datasheet-badge-null = NULL
datasheet-badge-list = lista
datasheet-badge-encoded = codificado
datasheet-badge-history = histórico
datasheet-null-context = Valor NULL
datasheet-stored-context = Resultado calculado armazenado; nenhuma fórmula avaliada; atualização desconhecida
datasheet-stored-null-context = Resultado calculado NULL armazenado; nenhuma fórmula avaliada; atualização desconhecida
datasheet-control-uninspected = As configurações de exibição do campo não foram inspecionadas; exibindo o valor armazenado
datasheet-control-absent = Nenhum controle de exibição foi declarado; exibindo o valor armazenado
datasheet-control-unsupported = As configurações de exibição do campo não são compatíveis; exibindo o valor armazenado
datasheet-control-textbox = Este campo usa uma caixa de texto
datasheet-control-lookup = A exibição de pesquisa ainda não é compatível; exibindo o valor armazenado
datasheet-context-pair = { $base }; { $presentation }

datasheet-go = Ir

datasheet-null-value = NULL
datasheet-close-table = Fechar tabela
datasheet-column-menu = Menu da coluna { $column }

datasheet-last = Último

datasheet-value-preview-truncated = Prévia abreviada. Copiar preserva o valor exibido completo.

datasheet-row-identity-invalid = As identidades dos registros não correspondem a esta página.
datasheet-filter-page-unavailable = Somente os registros filtrados carregados estão disponíveis aqui.

datasheet-click-to-add = Clique para adicionar

datasheet-add-field-unavailable = A adição de campos ainda não está disponível.

datasheet-record-position = Número do registro

datasheet-record-of = de { $count }

datasheet-count-unknown = ?

datasheet-search-records = Pesquisar registros

datasheet-no-filter = Sem filtro

datasheet-filtered = Filtrado
datasheet-page-tools = Ferramentas de página

# Saved-table Number field review and checked reopen.
datasheet-number-inspecting = Verificando se um campo Número pode ser adicionado…
datasheet-number-reviewing = Revisando o novo campo Número…
datasheet-number-reopening = Reabrindo a tabela salva…
datasheet-number-retry-inspection = Tentar novamente a verificação do campo

# Saved Number field close confirmation
datasheet-number-close-title = Salvar alterações no campo?
datasheet-number-close-question = Salvar o novo campo “{ $column }” na tabela “{ $table }”?
datasheet-number-close-stale = O nome do campo ou a tabela mudou. Feche esta caixa de diálogo e tente novamente.
datasheet-number-close-unavailable = Não é possível salvar este campo agora.
datasheet-number-close-save = Salvar
datasheet-number-close-discard = Não salvar
datasheet-number-close-cancel = Cancelar

# Interrupted readonly field eligibility observation.
datasheet-number-inspection-interrupted = A verificação do campo foi interrompida. Tente novamente para verificar esta tabela.

# Known saved field after failed checked reopening.
datasheet-number-saved-warning = O campo foi salvo, mas não foi possível verificar o banco de dados. Reabra o banco antes de salvar novamente.
datasheet-number-abandon-title = Fechar rascunho do campo?
datasheet-number-abandon-question = O campo foi salvo. Fechar este rascunho não desfaz esse salvamento.
datasheet-number-abandon-discard = Fechar rascunho
