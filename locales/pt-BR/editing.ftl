# TableCraft UI: editing.

editing-title = Edição de célula offline
editing-exclusive-help = Edição offline exclusiva. Feche este arquivo no Access e em outros aplicativos antes de aplicar.
editing-checking = Verificando a célula e lendo seu valor original…
editing-original = Valor armazenado original
editing-replacement = Substituição
editing-use-null = Usar NULL (sujeito à validação)
editing-validating = Validando a edição completa…
editing-reviewed = Alteração revisada
editing-before = Antes
editing-after = Depois
editing-changed-rows = Linhas alteradas: { $count }
editing-apply = Aplicar ao arquivo original
editing-review = Revisar alteração
editing-resolving = Resolvendo os recursos de salvamento retidos. Mantenha esta janela aberta até conhecer o resultado.
editing-applying = Aplicando a edição revisada. Mantenha esta janela aberta até conhecer o resultado.
editing-cancellation-requested = Cancelamento solicitado; aguardando o resultado real do salvamento.
editing-request-cancellation = Solicitar cancelamento
editing-recovery-owned = A sessão ainda mantém recursos de recuperação. Resolva-a antes de continuar.
editing-recover = Recuperar salvamento
editing-release = Liberar recursos de salvamento
editing-diagnosis-required = É necessário diagnosticar; a recuperação e a liberação automáticas não estão disponíveis.
editing-target = { $table } · { $column } · linha { $row }
editing-stored-value = Valor armazenado

editing-mock-rejected = A validação simulada rejeitou este valor de exemplo. Nenhum banco de dados foi aberto ou alterado.

editing-mock-failed = O salvamento simulado falhou. Nenhum banco de dados foi aberto ou alterado.

editing-mock-diagnosis = O resultado simulado exige diagnóstico. Não existe arquivo de banco de dados nem diário nesta prévia.

editing-outcome-saved = Salvamento concluído

editing-outcome-cancelled = Salvamento cancelado

editing-outcome-failed = É necessário verificar o salvamento

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = confirmado
editing-save-known-rolled-back = revertido
editing-save-known-original = imagem original verificada
editing-save-known-replacement = imagem substituta verificada
editing-save-known-unknown = não estabelecido
editing-save-completion-applied = aplicada
editing-save-completion-recovered = recuperada
editing-save-completion-confirmed = confirmada
editing-save-completed = O salvamento foi concluído e a liberação do bloqueio foi confirmada.
editing-save-released = Recursos liberados. Arquivos de recuperação podem permanecer; resolva-os antes de usar este arquivo. { $details }
editing-save-current = Resultado da operação atual: { $outcome }.
editing-save-no-receipt = Não há comprovante de operação concluída.
editing-save-prior = Última operação concluída: { $completion }; resultado: { $outcome }.
editing-save-matched = Linhas correspondentes: { $count }.
editing-save-changed = Linhas alteradas: { $count }.
editing-save-failure = O salvamento não terminou normalmente ({ $reason }). { $details } Não tente novamente automaticamente.
editing-save-reason-stale = o documento exibido foi substituído
editing-save-reason-open = não foi possível abrir o arquivo para edição exclusiva
editing-save-reason-source = o arquivo foi alterado após a visualização
editing-save-reason-cancelled = o cancelamento foi solicitado
editing-save-reason-resource = um limite da operação foi atingido
editing-save-reason-planning = não foi possível validar a edição
editing-save-reason-intent = não foi possível preparar o registro da alteração
editing-save-reason-setup = não foi possível configurar os arquivos de recuperação
editing-save-reason-prepare = não foi possível preparar o salvamento
editing-save-reason-commit = a confirmação não terminou normalmente
editing-save-reason-recovery = a recuperação não terminou normalmente
editing-save-reason-finalization = a finalização do salvamento não terminou normalmente
editing-save-reason-release = a liberação do bloqueio não foi confirmada
editing-save-reason-state = a sessão de salvamento requer diagnóstico

# Typed input instructions; literal true/false and ISO syntax stay invariant.
editing-input-boolean = Booleano: true ou false
editing-input-byte = Byte: de 0 a 255
editing-input-int16 = Inteiro de 16 bits com sinal
editing-input-int32 = Inteiro de 32 bits com sinal
editing-input-int64 = Inteiro de 64 bits com sinal
editing-input-single = Single: número decimal Use "." como separador decimal.
editing-input-double = Double: número decimal Use "." como separador decimal.
editing-input-currency = Moeda: valor decimal (quatro casas decimais) Use "." como separador decimal.
editing-input-decimal = Número decimal Use "." como separador decimal.
editing-input-text = Texto literal
editing-input-binary = Binário: pares de dígitos hexadecimais por byte
editing-input-guid = GUID: identificador canônico
editing-input-date = DATE legado: data e hora civil ISO (sem fuso horário)
editing-input-date-extended = Data e hora estendida: data e hora civil ISO (precisão de 100 ns)

editing-paste-text = Colar texto
editing-paste-help = Cole texto literal em uma célula nativa selecionada e revise-o. Use o formato de entrada invariável do campo; valores formatados e formatos tipados da área de transferência do Access não são interpretados. O texto com várias linhas permanece em uma célula.
editing-paste-too-large = O texto da área de transferência excede o limite de 1 MiB. Nada foi colado.
editing-paste-stale = A solicitação da área de transferência não corresponde mais à célula selecionada. O texto foi descartado.
editing-paste-multiple = A área de transferência retornou vários textos. Nada foi colado.

editing-save-retained-private-journal = Um diário de recuperação privado permanece no disco após salvar.

# An actor ended without a known current native outcome.
editing-save-uncertain = O processo de salvamento parou inesperadamente. O arquivo pode já ter sido alterado. Reinicie o TableCraft e reabra o banco de dados antes de fazer alterações; examine qualquer diário de recuperação mantido. Não há confirmação de reversão nem de nova tentativa.

editing-toggle-stale = A caixa de seleção não corresponde mais à célula verificada. A alteração proposta foi descartada.
