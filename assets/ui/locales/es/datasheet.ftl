# TableCraft UI: datasheet.

datasheet-column-count = Columnas: { $count }
datasheet-previous-page = ‹ Página anterior
datasheet-next-page = Página siguiente ›
datasheet-row-range = Filas { $first }–{ $last }
datasheet-no-rows = No hay filas.
datasheet-field-name = Nombre del campo: { $name }
datasheet-select-record = Seleccionar registro { $number }
datasheet-copy-record = Copiar registro mostrado
datasheet-record-selected-help = El registro { $number } está seleccionado. Se copian todos los campos mostrados y los resúmenes de colecciones, sin datos secundarios.
datasheet-select-record-help = Selecciona el registro { $number }. Se copian todos los campos mostrados y los resúmenes de colecciones, sin datos secundarios.
datasheet-view-contents = Ver contenido
datasheet-edit-cell = Editar celda…
datasheet-cell-context = { $column }, fila { $row }: { $text }
datasheet-cell-details = { $column }, fila { $row }: { $text }; { $context }
datasheet-copy-cell-help = Copiar incluye el valor completo de la celda.
datasheet-copy-incomplete = El registro seleccionado no contiene todos los campos mostrados.
datasheet-copy-limit = El registro mostrado supera el límite de 1 MiB del portapapeles.
datasheet-copy-allocation = No se pudo asignar memoria para el portapapeles.
datasheet-copy-encoding = No se pudo codificar el registro mostrado para el portapapeles.
datasheet-record = Registro:
datasheet-first = Primero
datasheet-previous = Anterior
datasheet-next = Siguiente
datasheet-go-record = Ir al registro
datasheet-record-input-help = Introduce un número entero de registro entre 1 y 4294967296.
datasheet-wrong-source = La página recibida pertenece a otro origen.
datasheet-wrong-offset = La página recibida empieza en otra posición de registro.
datasheet-missing-column = La columna solicitada no está en la página recibida.
datasheet-missing-record = No hay ningún registro en esa posición de la instantánea.
datasheet-incomplete-page = La página recibida no contiene el registro solicitado.
datasheet-badge-stored = almacenado
datasheet-badge-null = NULL
datasheet-badge-list = lista
datasheet-badge-encoded = codificado
datasheet-badge-history = historial
datasheet-null-context = Valor NULL
datasheet-stored-context = Resultado calculado almacenado; fórmula no evaluada; vigencia desconocida
datasheet-stored-null-context = Resultado calculado NULL almacenado; fórmula no evaluada; vigencia desconocida
datasheet-control-uninspected = La presentación del campo no se ha inspeccionado; se muestra el valor almacenado
datasheet-control-absent = No se ha declarado un control de presentación; se muestra el valor almacenado
datasheet-control-unsupported = Presentación del campo no compatible; se muestra el valor almacenado
datasheet-control-textbox = Este campo utiliza un cuadro de texto
datasheet-control-lookup = La presentación de búsqueda aún no es compatible; se muestra el valor almacenado
datasheet-context-pair = { $base }; { $presentation }

datasheet-go = Ir

datasheet-null-value = NULL
datasheet-close-table = Cerrar tabla
datasheet-column-menu = Menú de columna { $column }

datasheet-last = Último

datasheet-value-preview-truncated = Vista previa abreviada. Copiar conserva el valor mostrado completo.

datasheet-row-identity-invalid = Las identidades de los registros no coinciden con esta página.
datasheet-filter-page-unavailable = Aquí solo están disponibles los registros filtrados cargados.

datasheet-click-to-add = Haga clic para agregar

datasheet-add-field-unavailable = La adición de campos aún no está disponible.

datasheet-record-position = Número de registro

datasheet-record-of = de { $count }

datasheet-count-unknown = ?

datasheet-search-records = Buscar registros

datasheet-no-filter = Sin filtro

datasheet-filtered = Filtrado
datasheet-page-tools = Herramientas de página

# Saved-table Number field review and checked reopen.
datasheet-number-inspecting = Comprobando si se puede agregar un campo Número…
datasheet-number-reviewing = Revisando el nuevo campo Número…
datasheet-number-reopening = Volviendo a abrir la tabla guardada…
datasheet-number-retry-inspection = Reintentar la comprobación del campo

# Saved Number field close confirmation
datasheet-number-close-title = ¿Guardar los cambios del campo?
datasheet-number-close-question = ¿Guardar el nuevo campo «{ $column }» en la tabla «{ $table }»?
datasheet-number-close-stale = El nombre del campo o la tabla ha cambiado. Cierre este diálogo e inténtelo de nuevo.
datasheet-number-close-unavailable = Este campo no se puede guardar ahora.
datasheet-number-close-save = Guardar
datasheet-number-close-discard = No guardar
datasheet-number-close-cancel = Cancelar

# Interrupted readonly field eligibility observation.
datasheet-number-inspection-interrupted = La comprobación del campo se interrumpió. Reinténtela para comprobar esta tabla.
