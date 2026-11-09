# Nombres de campos guardados: comprobación, revisión y reapertura verificada.

field-rename-title = Cambiar nombre del campo
field-rename-name = Nombre del campo
field-rename-design-help = Use «{ field-rename-title }» para proponer un nombre de campo. Las demás propiedades del campo siguen siendo de solo lectura.
field-rename-original = Campo original: { $column }
field-rename-proposed = Nombre propuesto: { $column }
field-rename-inspecting = Comprobando si se puede cambiar el nombre de este campo…
field-rename-inspection-interrupted = Se interrumpió la comprobación del campo. Vuelva a intentarlo para comprobarlo de nuevo.
field-rename-retry-inspection = Reintentar comprobación del campo
field-rename-source-unavailable = El cambio de nombre de este campo no está disponible para esta base de datos.
field-rename-profile =
    { $units ->
        [one] Por ahora, el nombre del campo debe usar ASCII y tener exactamente { $units } carácter.
       *[other] Por ahora, el nombre del campo debe usar ASCII y tener exactamente { $units } caracteres.
    }
field-rename-reviewing = Revisando el nombre del campo…
field-rename-reopening = Reabriendo y verificando el campo con el nuevo nombre…
field-rename-unchanged-reopening = Comprobando la base de datos sin cambios…
field-rename-unchanged = No se escribió ningún cambio de nombre de campo.
field-rename-cancel = Cancelar cambio de nombre
field-rename-return = Volver al editor de nombres
field-rename-close-title = Cambios en el nombre del campo
field-rename-close-question = ¿Guardar los cambios en el nombre de { $column } en { $table } antes de cerrar?
field-rename-close-save = Guardar
field-rename-close-discard = Descartar
field-rename-close-cancel = Cancelar
field-rename-close-unavailable = No se puede guardar este nombre propuesto. Descártelo o vuelva al editor de nombres.
field-rename-close-stale = El nombre propuesto ha cambiado. Cancele e intente cerrar de nuevo.
field-rename-close-confirm = ¿Cerrar el editor de nombres? Esto no deshace los cambios ya escritos en el archivo.
field-rename-warning-saved = Se escribió el nombre del campo, pero no se pudo verificar la base de datos. Vuelva a abrirla antes de realizar más cambios.
field-rename-warning-unchanged = No se escribió ningún cambio de nombre de campo, pero no se pudo verificar la base de datos. Vuelva a abrirla antes de realizar más cambios.
field-rename-close-editor = Cerrar editor de nombres
