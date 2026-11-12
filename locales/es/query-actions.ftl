# Reviewed SQL actions; runtime owns native writes.
query-action-update-title = Consulta de actualización
query-action-update-reviewing = Revisando la consulta de actualización…
query-action-table = Tabla: { $table }
query-action-matched = Registros coincidentes: { $count }
query-action-update-changed = Registros con cambios efectivos: { $count }
query-action-update-confirmation = ¿Aplicar estos cambios revisados a la base de datos original?
query-action-no-undo = No se puede deshacer esta operación.
query-action-yes = Sí
query-action-no = No
query-action-invalid-review = La acción revisada ya no coincide con este borrador SQL.
query-action-delete-title = Consulta de eliminación
query-action-delete-reviewing = Revisando la consulta de eliminación…
query-action-delete-deleted = Registros que se eliminarán: { $count }
query-action-delete-confirmation = ¿Eliminar permanentemente estos registros revisados de la base de datos original?
query-action-insert-title = Consulta de datos anexados
query-action-insert-reviewing = Revisando la consulta de datos anexados…
query-action-insert-inserted = Registros que se anexarán: { $count }
query-action-insert-confirmation = ¿Anexar los registros revisados a la base de datos original?

## Reviewed SQL schema changes
query-action-schema-title = Consulta de definición de datos
query-action-schema-reviewing = Revisando la consulta de esquema…
query-action-schema-confirmation = ¿Aplicar este cambio revisado a la estructura de la base de datos?
query-action-schema-create-fields = Campos que se crearán: { $count }
query-action-schema-drop-records = Registros en la tabla que se eliminará: { $count }
query-action-schema-drop-field = Campo que se eliminará: { $column }
query-action-schema-create-confirmation = ¿Crear esta tabla revisada en la base de datos original?
query-action-schema-drop-table-confirmation = ¿Eliminar permanentemente esta tabla revisada y sus registros de la base de datos original?
query-action-schema-drop-column-confirmation = ¿Eliminar permanentemente este campo revisado y sus valores de la base de datos original?
query-action-schema-close-target = Cierre la tabla de destino antes de ejecutar esta consulta de esquema.
query-action-schema-reopen-failed = El cambio de esquema se guardó, pero no se pudo volver a abrir la base de datos. No repita la consulta.
query-action-schema-retry-reopen = Volver a abrir
query-action-schema-retained-results = Se conservan los resultados anteriores. Vuelva a abrir la base de datos guardada para actualizar su estructura; no repita la consulta.

query-action-schema-make-table-confirmation = ¿Crear esta tabla e insertar todos los registros seleccionados en una sola operación?
query-action-schema-make-table-records = Registros seleccionados para insertar: { $count }

query-action-schema-index-name = Índice: { $name }
query-action-schema-index-field = Campo: { $name }
query-action-schema-index-ascending = Orden: Ascendente
query-action-schema-index-descending = Orden: Descendente
query-action-schema-index-source-rows = Registros existentes para indizar: { $count }
query-action-schema-index-confirmation = ¿Crear este índice no único en la base de datos original?

query-action-schema-add-confirmation = ¿Agregar este campo a la tabla original?

query-action-schema-drop-index-name = Índice que se quitará: { $name }
query-action-schema-drop-index-confirmation = ¿Quitar permanentemente este índice de la base de datos original? Los valores de los registros permanecerán sin cambios.
