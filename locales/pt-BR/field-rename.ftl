# Nomes de campos salvos: verificação, revisão e reabertura verificada.

field-rename-title = Renomear Campo
field-rename-name = Nome do campo
field-rename-design-help = Use { field-rename-title } para propor um nome de campo. As outras propriedades do campo continuam somente leitura.
field-rename-original = Campo original: { $column }
field-rename-proposed = Nome proposto: { $column }
field-rename-inspecting = Verificando se este campo pode ser renomeado…
field-rename-inspection-interrupted = A verificação do campo foi interrompida. Tente novamente para verificar este campo.
field-rename-retry-inspection = Repetir verificação do campo
field-rename-source-unavailable = A renomeação deste campo não está disponível para este banco de dados.
field-rename-profile =
    { $units ->
        [one] Por enquanto, o nome do campo deve usar ASCII e ter exatamente { $units } caractere.
       *[other] Por enquanto, o nome do campo deve usar ASCII e ter exatamente { $units } caracteres.
    }
field-rename-reviewing = Revisando o nome do campo…
field-rename-reopening = Reabrindo e verificando o campo renomeado…
field-rename-unchanged-reopening = Verificando o banco de dados sem alterações…
field-rename-unchanged = Nenhuma alteração no nome do campo foi gravada.
field-rename-cancel = Cancelar renomeação
field-rename-return = Voltar ao editor de nomes
field-rename-close-title = Alterações no nome do campo
field-rename-close-question = Salvar as alterações no nome de { $column } em { $table } antes de fechar?
field-rename-close-save = Salvar
field-rename-close-discard = Descartar
field-rename-close-cancel = Cancelar
field-rename-close-unavailable = Não é possível salvar este nome proposto. Descarte as alterações ou volte ao editor de nomes.
field-rename-close-stale = O nome proposto mudou. Cancele o fechamento e tente novamente.
field-rename-close-confirm = Fechar o editor de nomes? Isso não desfaz as alterações já gravadas no arquivo.
field-rename-warning-saved = O nome do campo foi gravado, mas não foi possível verificar o banco de dados. Reabra o banco de dados antes de fazer outras alterações.
field-rename-warning-unchanged = Nenhuma alteração no nome do campo foi gravada, mas não foi possível verificar o banco de dados. Reabra o banco de dados antes de fazer outras alterações.
field-rename-close-editor = Fechar editor de nomes
