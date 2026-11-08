# Unsaved native-table authoring. No persisted table is implied.
table-draft-unavailable = La tabla sin guardar ya no está disponible.
table-draft-new-record = (Nuevo)
table-draft-short-text = Texto corto (255)
table-draft-field-name = Nombre del campo
table-draft-invalid-name = Elija un nombre de campo válido y único.
table-draft-empty-note = Guarde esta tabla para continuar.
table-draft-save = Guardar
table-draft-save-unavailable = Guardar esta tabla no está disponible en este momento. Mantenga el borrador abierto o descártelo.
table-draft-close-title = Tabla sin guardar
table-draft-close-question = ¿Guardar los cambios en la tabla sin guardar?
table-draft-discard = Descartar
table-draft-cancel = Cancelar

table-draft-save-as = Guardar como
table-draft-table-name = Nombre de la tabla
table-draft-invalid-table-name = Elija un nombre de tabla válido de hasta 64 caracteres.
table-draft-ok = Aceptar
table-draft-save-cancelled = Se canceló el guardado. El borrador de la tabla no ha cambiado.
table-draft-save-failed = No se pudo guardar la tabla.
table-draft-save-retained = El resultado del guardado necesita recuperación o verificación. Mantenga esta ventana abierta.
table-draft-save-working = Guardando tabla…
table-draft-save-cancelling = Cancelando el guardado…
table-draft-save-stale = El guardado ya no pertenece a este borrador de tabla.
table-draft-save-blocked = El guardado sigue bloqueado tras un resultado sin resolver. Vuelva a abrir la base de datos para revisarla o descarte este borrador.
table-draft-save-reopen-failed = La tabla se guardó, pero no se pudo volver a abrir. Vuelva a abrir la base de datos antes de guardar este borrador de nuevo.

# Click-to-Add declarations; unsupported options remain visible and disabled.
table-draft-large-integer = Número grande
table-draft-currency = Moneda
table-draft-date-time = Fecha y hora
table-draft-yes-no = Sí/No
table-draft-long-text = Texto largo
table-draft-lookup-relationship = Búsqueda y relación
table-draft-rich-text = Texto enriquecido
table-draft-attachment = Datos adjuntos
table-draft-hyperlink = Hipervínculo
table-draft-calculated-field = Campo calculado
table-draft-more-fields = Más campos
table-draft-record-field = Nuevo registro { $field }
table-draft-record-schema-saved = La tabla se ha guardado; este registro no se ha guardado.

table-record-preparing = La tabla se guardó. Preparando el nuevo registro…
table-record-retry = Reintentar nuevo registro
table-record-discard = Descartar nuevo registro
table-record-invalid = La tabla se guardó, pero no se pudo preparar el nuevo registro. Se conserva lo introducido.

# Initial unsaved fields only; deleting also discards their pending record input.
table-draft-field-options = Opciones del campo { $field }
table-draft-rename-field = Cambiar nombre del campo
table-draft-delete-field = Eliminar campo
table-draft-delete-field-title = Eliminar campo sin guardar
table-draft-delete-field-question = ¿Eliminar el campo sin guardar «{ $field }»?
table-draft-delete-field-warning = Se eliminarán el campo y sus datos de registro pendientes de esta tabla sin guardar. La base de datos no se ha modificado.
table-draft-delete-field-confirm = Eliminar campo

# Editable initial-table Design view; persisted native schemas remain read only.
table-draft-design-help = Edita los nombres, tipos y propiedades de campo compatibles de esta tabla sin guardar. Las demás propiedades aún no están disponibles.
table-draft-design-name = Nombre del campo { $field }
table-draft-design-type = Tipo de datos de { $field }
table-draft-design-input-help = Los datos de registro pendientes se conservan tal como se introdujeron al cambiar el tipo. Deben ser válidos para el tipo seleccionado antes de revisar y aplicar.
table-draft-design-protected = La clave principal Autonumeración inicial no se puede cambiar.
table-draft-design-add-field = Agregar campo

# Supported initial-field properties; default input is an unevaluated constant.
table-draft-property-unset = Sin establecer
table-draft-property-no = No
table-draft-property-yes = Sí
table-draft-property-type-default = Predeterminado del tipo
table-draft-property-absent = Sin valor predeterminado
table-draft-property-null = NULL
table-draft-property-literal = Literal
table-draft-property-literal-input = Valor literal predeterminado
table-draft-property-invalid-size = Introduce un número entero entre 1 y 255.
table-draft-property-input-limit = La entrada de la propiedad supera su límite. Se conserva el texto anterior.
table-draft-property-invalid = Corrige las propiedades de campo no válidas antes de guardar.
table-draft-property-help = Solo constantes: texto sin comillas SQL, números con punto decimal, true/false o fecha y hora en formato ISO. No se evalúan fórmulas. La entrada admite 4096 bytes UTF-8; el valor predeterminado guardado debe caber en 255 unidades UTF-16, incluidas las comillas añadidas y escapadas.

table-draft-number = Número
table-draft-number-byte = Byte
table-draft-number-integer = Entero
table-draft-number-single = Simple
table-draft-number-double = Doble
table-draft-number-decimal = Decimal
table-draft-number-replication-id = Id. de replicación
table-draft-number-precision = Precisión
table-draft-number-scale = Escala
table-draft-number-invalid-decimal = La precisión debe ser de 1 a 28 y la escala de 0 a la precisión. Corrija ambos valores antes de guardar.
table-draft-number-guid-default = Los valores predeterminados literales de Id. de replicación aún no están disponibles. Elija otro modo; el texto introducido se conserva.

table-draft-design-description = Descripción de { $field }
table-draft-description-invalid = La descripción del campo es demasiado larga.

table-draft-no-key-title = Sin clave principal
table-draft-no-key-question = No se ha definido una clave principal. ¿Crear una clave principal de Autonumeración?
table-draft-no-key-failed = No se pudo agregar la clave principal. Conserve el diseño actual o libere un campo.

table-draft-design-new-name = Nombre del nuevo campo
table-draft-design-new-type = Tipo de datos del nuevo campo
