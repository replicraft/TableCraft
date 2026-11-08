# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = A tabela não salva não está mais disponível.
table-draft-new-record = (Novo)
table-draft-short-text = Texto Curto (255)
table-draft-field-name = Nome do campo
table-draft-invalid-name = Escolha um nome de campo válido e exclusivo.
table-draft-empty-note = Salve esta tabela para continuar.
table-draft-save = Salvar
table-draft-save-unavailable = Não é possível salvar esta tabela no momento. Mantenha o rascunho aberto ou descarte-o.
table-draft-close-title = Tabela não salva
table-draft-close-question = Salvar alterações na tabela não salva?
table-draft-discard = Descartar
table-draft-cancel = Cancelar

table-draft-save-as = Salvar como
table-draft-table-name = Nome da tabela
table-draft-invalid-table-name = Escolha um nome de tabela válido com até 64 caracteres.
table-draft-ok = OK
table-draft-save-cancelled = O salvamento foi cancelado. O rascunho da tabela não foi alterado.
table-draft-save-failed = Não foi possível salvar a tabela.
table-draft-save-retained = O resultado do salvamento precisa de recuperação ou verificação. Mantenha esta janela aberta.
table-draft-save-working = Salvando tabela…
table-draft-save-cancelling = Cancelando salvamento…
table-draft-save-stale = O salvamento não pertence mais a este rascunho de tabela.
table-draft-save-blocked = O salvamento continua bloqueado após um resultado não resolvido. Reabra o banco para inspecioná-lo ou descarte este rascunho.
table-draft-save-reopen-failed = A tabela foi salva, mas não foi possível concluir a reabertura. Reabra o banco de dados antes de salvar este rascunho novamente.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = Número Grande
table-draft-currency = Moeda
table-draft-date-time = Data e Hora
table-draft-yes-no = Sim/Não
table-draft-long-text = Texto Longo
table-draft-lookup-relationship = Pesquisa e Relação
table-draft-rich-text = Rich Text
table-draft-attachment = Anexo
table-draft-hyperlink = Hiperlink
table-draft-calculated-field = Campo Calculado
table-draft-more-fields = Mais Campos
table-draft-record-field = Novo registro { $field }
table-draft-record-schema-saved = A tabela foi salva; este registro ainda não foi salvo.

table-record-preparing = A tabela foi salva. Preparando o novo registro…
table-record-retry = Tentar novo registro novamente
table-record-discard = Descartar novo registro
table-record-invalid = A tabela foi salva, mas não foi possível preparar o novo registro. Os dados inseridos foram mantidos.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = Opções do campo { $field }
table-draft-rename-field = Renomear campo
table-draft-delete-field = Excluir campo
table-draft-delete-field-title = Excluir campo não salvo
table-draft-delete-field-question = Excluir o campo não salvo “{ $field }”?
table-draft-delete-field-warning = Isso remove o campo e os dados pendentes de seu registro desta tabela não salva. O banco de dados não foi alterado.
table-draft-delete-field-confirm = Excluir campo

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = Edite nomes, tipos e propriedades de campo compatíveis nesta tabela não salva. As demais propriedades ainda não estão disponíveis.
table-draft-design-name = Nome do campo { $field }
table-draft-design-type = Tipo de dados de { $field }
table-draft-design-input-help = Os dados pendentes do registro são mantidos como foram digitados quando o tipo muda. Eles devem ser válidos para o tipo selecionado antes de revisar e aplicar.
table-draft-design-protected = A chave primária inicial de Numeração Automática não pode ser alterada.
table-draft-design-add-field = Adicionar campo

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = Não definido
table-draft-property-no = Não
table-draft-property-yes = Sim
table-draft-property-type-default = Padrão do tipo
table-draft-property-absent = Sem valor padrão
table-draft-property-null = NULL
table-draft-property-literal = Literal
table-draft-property-literal-input = Literal do valor padrão
table-draft-property-invalid-size = Digite um número inteiro entre 1 e 255.
table-draft-property-input-limit = A entrada da propriedade excede o limite. O texto anterior foi mantido.
table-draft-property-invalid = Corrija as propriedades de campo inválidas antes de salvar.
table-draft-property-help = Somente constantes: texto sem aspas SQL, números com ponto decimal, true/false ou data e hora no formato ISO. Fórmulas não são avaliadas. A entrada permite 4096 bytes UTF-8; o padrão salvo deve caber em 255 unidades UTF-16, incluindo aspas adicionadas e escapadas.

table-draft-number = Número
table-draft-number-byte = Byte
table-draft-number-integer = Inteiro
table-draft-number-single = Simples
table-draft-number-double = Duplo
table-draft-number-decimal = Decimal
table-draft-number-replication-id = ID de replicação
table-draft-number-precision = Precisão
table-draft-number-scale = Escala
table-draft-number-invalid-decimal = A precisão deve ser de 1 a 28 e a escala de 0 até a precisão. Corrija os dois valores antes de salvar.
table-draft-number-guid-default = Valores padrão literais para ID de replicação ainda não estão disponíveis. Escolha outro modo; o texto inserido será mantido.

table-draft-design-description = Descrição de { $field }
table-draft-description-invalid = A descrição do campo é muito longa.

table-draft-no-key-title = Sem chave primária
table-draft-no-key-question = Nenhuma chave primária foi definida. Criar uma chave primária de Numeração Automática?
table-draft-no-key-failed = Não foi possível adicionar a chave primária. Mantenha o design atual ou remova um campo.

table-draft-design-new-name = Nome do novo campo
table-draft-design-new-type = Tipo de dados do novo campo
