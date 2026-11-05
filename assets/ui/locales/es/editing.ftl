# TableCraft UI: editing.

editing-title = Editar celda sin conexión
editing-exclusive-help = Edición exclusiva sin conexión. Cierra este archivo en Access y otras aplicaciones antes de aplicar los cambios.
editing-checking = Comprobando la celda y leyendo su valor original…
editing-original = Valor almacenado original
editing-replacement = Reemplazo
editing-use-null = Usar NULL (sujeto a validación)
editing-validating = Validando el cambio completo…
editing-reviewed = Cambio revisado
editing-before = Antes
editing-after = Después
editing-changed-rows = Filas modificadas: { $count }
editing-apply = Aplicar al archivo original
editing-review = Revisar cambio
editing-resolving = Resolviendo los recursos de guardado retenidos. Mantén esta ventana abierta hasta conocer el resultado.
editing-applying = Aplicando el cambio revisado. Mantén esta ventana abierta hasta conocer el resultado.
editing-cancellation-requested = Cancelación solicitada; esperando el resultado real del guardado.
editing-request-cancellation = Solicitar cancelación
editing-recovery-owned = La sesión conserva recursos de recuperación. Resuélvela antes de continuar.
editing-recover = Recuperar guardado
editing-release = Liberar recursos de guardado
editing-diagnosis-required = Se requiere diagnóstico; la recuperación y liberación automáticas no están disponibles.
editing-target = { $table } · { $column } · fila { $row }
editing-stored-value = Valor almacenado

editing-mock-rejected = La validación simulada rechazó este valor de ejemplo. No se abrió ni modificó ninguna base de datos.

editing-mock-failed = Error al guardar la simulación. No se abrió ni modificó ninguna base de datos.

editing-mock-diagnosis = El resultado simulado requiere un diagnóstico. No existe ningún archivo de base de datos ni diario en esta vista previa.

editing-outcome-saved = Guardado completado

editing-outcome-cancelled = Guardado cancelado

editing-outcome-failed = El guardado requiere atención

# Host-authored save status; service payloads remain literal.
editing-save-known-committed = confirmado
editing-save-known-rolled-back = revertido
editing-save-known-original = imagen original verificada
editing-save-known-replacement = imagen de reemplazo verificada
editing-save-known-unknown = no establecido
editing-save-completion-applied = aplicada
editing-save-completion-recovered = recuperada
editing-save-completion-confirmed = confirmada
editing-save-completed = El guardado finalizó y se confirmó la liberación del bloqueo.
editing-save-released = Recursos liberados. Pueden quedar archivos de recuperación; resuélvalos antes de usar este archivo. { $details }
editing-save-current = Resultado de la operación actual: { $outcome }.
editing-save-no-receipt = No hay recibo de una operación completada.
editing-save-prior = Última operación completada: { $completion }; resultado: { $outcome }.
editing-save-matched = Filas coincidentes: { $count }.
editing-save-changed = Filas modificadas: { $count }.
editing-save-failure = El guardado no finalizó normalmente ({ $reason }). { $details } No reintente automáticamente.
editing-save-reason-stale = se reemplazó el documento mostrado
editing-save-reason-open = no se pudo abrir el archivo para edición exclusiva
editing-save-reason-source = el archivo cambió después de visualizarlo
editing-save-reason-cancelled = se solicitó la cancelación
editing-save-reason-resource = se alcanzó un límite de operación
editing-save-reason-planning = no se pudo validar la edición
editing-save-reason-intent = no se pudo preparar el registro del cambio
editing-save-reason-setup = no se pudieron configurar los archivos de recuperación
editing-save-reason-prepare = no se pudo preparar el guardado
editing-save-reason-commit = la confirmación no finalizó normalmente
editing-save-reason-recovery = la recuperación no finalizó normalmente
editing-save-reason-finalization = la finalización del guardado no terminó normalmente
editing-save-reason-release = no se confirmó la liberación del bloqueo
editing-save-reason-state = la sesión de guardado requiere diagnóstico
