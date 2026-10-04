# Spanish review — 2026-10-04

GPT-6.1 Sol High reviewed all 930 Spanish strings (`ui/src/locales/es.json`) against English, the terms table and how the code joins strings. Checked against the code by Claude: the "Listado en" join is a real bug in English too (`{why} Listed in {file}.` after a file error that has no full stop); "(ninguno)" follows "Plantillas de copia", so it should be feminine. Nothing is changed until approved.

Reviewer's text (in Spanish):

He revisado las **836 entradas —930 cadenas contando las formas plurales—** de [es.json](/Users/beli/conductor/workspaces/secopy-v1/hat-yai/ui/src/locales/es.json), comparándolas con el inglés, el glosario y las composiciones en Svelte y Rust. No he modificado archivos.

A continuación figuran los cambios que recomiendo, ordenados por prioridad. Las entradas no mencionadas pueden mantenerse. En las tablas, `one / other` indica ambas formas plurales.

**A) Incorrectas, ambiguas o potencialmente engañosas**

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `errors.file.inSource` | Caería sobre un archivo del origen | La copia sobrescribiría un archivo del origen | «Caería sobre» no explica el riesgo para los datos. |
| `summary.headline.check.unreadable` | `{count} ilegible / {count} ilegibles` | `No se pudo leer {count} archivo / No se pudieron leer {count} archivos` | Puede tratarse de permisos o desconexiones; «ilegible» parece atribuir el problema al contenido. |
| `queue.reason.part.unreadable` | `{count} ilegible / {count} ilegibles` | `No se pudo leer {count} archivo / No se pudieron leer {count} archivos` | Debe expresar la imposibilidad de leer, igual que el resumen. |
| `mirror.editor.days` | Días que conservar | Días de conservación | Se conservan archivos durante determinados días; no se conservan los días. |
| `mirror.guard.tooMany` | Se eliminarían `{removals}` de los `{files}` archivos del destino. | Archivos del destino que se eliminarían: `{removals}` de `{files}`. | La composición actual produce «de los 1 archivos» cuando el destino contiene uno. |
| `export.none` | (ninguno) | (ninguna) | Se inserta tras «Plantillas de copia» o «Plantillas de espejo»: el referente es femenino. |
| `mirror.archive.empty` | Vacío | Sin archivos | Aparece bajo «Archivados»; «Vacío» introduce un referente masculino singular inexistente. |
| `notify.queue.title` | `Cola terminada: {complete} de {count} tarea completa / … tareas completas` | Ambas formas: `Cola terminada. Tareas completadas: {complete} de {count}` | Separa el número completado del total y evita formulaciones como «0 de 1 tarea completa». |
| `summary.headline.cancelled` | Cancelado | Cancelada | El estado corresponde a una tarea, también cuando es una copia o una verificación. |
| `summary.headline.stopped` | Detenido: `{why}` | Tarea detenida: `{why}` | Corrige el género y hace explícito qué se ha detenido. |
| `progress.stopped` | Detenido: `{why}` | Tarea detenida: `{why}` | Mismo referente y mismo estado que en el resumen. |
| `progress.phase.done` | Terminado | Completada | Concuerda con «tarea» y distingue una tarea completada de una que simplemente ha terminado. |
| `menubar.ended.complete` | Terminado: todos los archivos procesados | Tarea completada: todos los archivos procesados | Evita el masculino y mantiene el significado de finalización satisfactoria. |
| `menubar.ended.failures` | Terminado con problemas | Tarea terminada con problemas | Concordancia con «tarea», sin sugerir que se completó correctamente. |
| `menubar.ended.cancelled` | Cancelado | Cancelada | Concuerda con la tarea cuyo resultado muestra el panel. |
| `menubar.ended.stopped` | Detenido | Detenida | Mismo criterio que en la cola. |
| `menubar.ended.stoppedBecause` | Detenido: `{why}` | Tarea detenida: `{why}` | Corrige el género conservando la causa. |
| `summary.stats.written` | `{size}` escritos | Datos escritos: `{size}` | Evita «1 B escritos»; `{size}` ya contiene cifra y unidad. |
| `copy.free` | `{size}` disponibles · `{kind}` | Disponible: `{size}` · `{kind}` | Evita concordancias incorrectas con una cantidad de un byte. |
| `copy.preflight.purgeable` | Necesita espacio purgable: `{needed}` necesarios, `{free}` disponibles ahora | Requiere espacio purgable. Espacio necesario: `{needed}`; disponible ahora: `{free}` | Las cantidades formateadas no permiten imponer siempre el plural. |
| `errors.blocker.notEnoughSpace` | No hay espacio suficiente: `{needed}` necesarios, `{available}` disponibles | No hay espacio suficiente. Espacio necesario: `{needed}`; disponible: `{available}` | Misma dependencia incorrecta de la cantidad y la unidad. |
| `menubar.left` | Quedan `{time}` | Tiempo restante: `{time}` | `{time}` es «0:01», no una expresión con unidad; «Quedan» resulta problemático con un segundo. |
| `settings.language.help` | Automático usa el idioma del Mac si Secopy lo tiene; si no, inglés. Las ventanas propias de macOS (Abrir, Guardar, Acerca de) cambian en el próximo arranque. | “Automático” usa el idioma del Mac si está disponible en Secopy; si no, usa el inglés. Los cuadros de diálogo de macOS (“Abrir”, “Guardar” y “Acerca de”) cambian la próxima vez que abras Secopy. | «Próximo arranque» puede entenderse como reiniciar el Mac; se refiere a volver a abrir la app. |
| `errors.os.stale` | El archivo de la unidad de red ya no es válido | La referencia al archivo de la unidad de red ya no es válida | Rust lo usa para `StaleNetworkFileHandle`: ha caducado la referencia, no necesariamente el archivo. |
| `dialog.secopyFiles` | Ajustes de Secopy | Archivos de Secopy | El filtro admite archivos con plantillas, aunque no contengan ajustes; la imprecisión también está en inglés. |

Las concordancias anteriores se han comprobado en el código. **No cambiaría** `finished.status.cancelled`: allí «Cancelado» se refiere a un **archivo** y es correcto.

**B) Poco naturales o inconsistentes**

Primero corregiría estas expresiones visibles y recurrentes:

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `summary.headline.mirror.done` | Espejo hecho: `{parts}` | Espejo actualizado: `{parts}` | Describe mejor el resultado de ejecutar un espejo existente. |
| `summary.headline.undoneBack` | Cancelado: el destino ha vuelto a como estaba | Tarea cancelada: se ha restaurado el estado anterior del destino | «Ha vuelto a como estaba» resulta poco pulido y no concuerda con «tarea». |
| `summary.headline.undoneRemoved` | Cancelado: no se pudo devolver todo a como estaba | Tarea cancelada: no se pudo restaurar por completo el estado anterior del destino | Explica con precisión que la restauración fue incompleta. |
| `progress.undoing` | Devolviendo el destino a como estaba… | Restaurando el estado anterior del destino… | Expresión natural y coherente con los resultados de la restauración. |
| `progress.quit.finishing.undo` | Secopy termina primero de devolver el destino a como estaba y luego sale. | Secopy terminará de restaurar el estado anterior del destino antes de salir. | La formulación actual es demasiado literal. |
| `progress.stop.alsoRemoveHelp` | Elimina lo que copió esta tarea. Lo que reemplazó solo vuelve desde los archivados de un espejo. | Elimina los archivos copiados por esta tarea. Los archivos reemplazados solo se pueden restaurar desde los archivados de un espejo. | «Lo que reemplazó solo vuelve» oculta una condición importante de recuperación. |
| `summary.notRestored` | `No se pudo restaurar {count} archivo que esta tarea reemplazó: se queda su versión nueva. / No se pudieron restaurar {count} archivos que esta tarea reemplazó: se quedan sus versiones nuevas.` | `No se pudo restaurar {count} archivo reemplazado por esta tarea: se conserva la versión nueva. / No se pudieron restaurar {count} archivos reemplazados por esta tarea: se conservan las versiones nuevas.` | «Se queda su versión nueva» suena conversacional y poco preciso. |
| `summary.stats.notCheckedHint` | En ningún archivo de checksums: no hay con qué compararlos. | Sin una entrada en un archivo de checksums, no hay un checksum con el que comparar el contenido. | La frase actual está truncada y «compararlos» presupone varios archivos. |
| `summary.stats.notStartedHint` | No alcanzados: la tarea se canceló o se detuvo. | No se llegó a procesar estos archivos porque la tarea se canceló o se detuvo. | «No alcanzados» es un calco poco natural en este contexto. |
| `summary.headline.nothingToCopy` | Nada que copiar: todo estaba ya allí | Nada que copiar: todo estaba ya en el destino | «En el destino» es más preciso que «allí». |
| `copy.status.blocked` | Algo de arriba impide la copia. | No se puede iniciar la copia. Consulta los problemas indicados arriba. | «Algo de arriba» resulta vago y poco propio de una interfaz cuidada. |
| `copy.to`, `menubar.to` | Hacia | A | Una copia va «de… a…»; «hacia» expresa dirección aproximada. |
| `presets.changed` | Cambiada para esta vez | Modificada para esta ejecución | Describe un cambio temporal de la plantilla con mayor precisión. |
| `mirror.notRemoved.notClean` | Los archivos borrados en el origen se dejaron en el destino: la copia no terminó limpiamente. | Los archivos borrados en el origen se conservaron en el destino: la copia no terminó correctamente. | «Terminar limpiamente» es un calco técnico. |
| `mirror.archiveNotCleanedAll` | No se pudieron limpiar los archivados (`{why}`). Secopy lo vuelve a intentar en la próxima ejecución. | No se pudieron eliminar los archivados cuyo plazo de conservación ha vencido (`{why}`). Secopy lo volverá a intentar en la próxima ejecución. | «Limpiar los archivados» no explica qué se intentó eliminar. |
| `errors.file.inTheWay` | Hay algo en medio en `{path}` | Un elemento en `{path}` impide la copia | «Algo en medio» resulta impreciso y demasiado coloquial. |
| `mirror.preview.failing` | `{count} archivo fallará: un nombre que el destino no admite, o algo en medio / {count} archivos fallarán: un nombre que el destino no admite, o algo en medio` | `No se podrá copiar {count} archivo: el destino no admite su nombre o hay un elemento que lo impide / No se podrán copiar {count} archivos: el destino no admite sus nombres o hay elementos que lo impiden` | Explica la operación que fallará y elimina «algo en medio». |
| `errors.os.notFound` | No está ahí | No se encuentra | Expresión habitual y válida para archivos o directorios. |
| `errors.mirror.originMissing` | El origen no está ahí: `{path}` | No se encuentra el origen: `{path}` | Evita la referencia espacial vaga. |
| `errors.source.gone` | `{path}` ya no está ahí. | Ya no se encuentra `{path}`. | Más natural como mensaje de error. |
| `errors.job.retrySourceGone` | El origen ya no está ahí (`{path}`). Vuelve a conectar el origen para reintentar. | Ya no se encuentra el origen (`{path}`). Vuelve a conectarlo para reintentar la copia. | Elimina la repetición de «origen» y concreta qué se reintenta. |
| `errors.queue.busy` | Ya hay una tarea o la cola en curso. | Ya hay una tarea en curso o la cola está en ejecución. | La coordinación «hay… o la cola» resulta forzada. |

En los estados de la cola usaría **«completada»**, reservando **«terminada»** para una ejecución que ha acabado, con independencia de su resultado:

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `queue.result.complete` | Completa | Completada | Es el resultado de ejecutar la tarea, no una descripción de su contenido. |
| `queue.done.headline` | Cola terminada. Tareas completas: `{complete}` de `{jobs}` | Cola terminada. Tareas completadas: `{complete}` de `{jobs}` | Coherencia con el estado individual y la notificación. |
| `notify.queue.allFinished` | Todas las tareas han terminado. | Todas las tareas se han completado. | El código solo lo muestra cuando todas tienen resultado `complete`. |

Para archivos cuyo contenido o metadatos han cambiado, **«modificado»** resulta más natural que **«cambiado»**. Recomiendo aplicarlo en estos lugares:

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `summary.headline.check.changed` | `{count} archivo cambiado / {count} archivos cambiados` | `{count} archivo modificado / {count} archivos modificados` | Vocabulario natural para archivos alterados desde la copia. |
| `finished.status.changed` | ✗ Cambiado | ✗ Modificado | Mismo estado que en el resumen. |
| `queue.reason.part.changed` | `{count} cambiado / {count} cambiados` | `{count} modificado / {count} modificados` | Coherencia entre cola y resumen. |
| `mirror.preview.copied.changed` | `{count} archivo cambiado / {count} archivos cambiados` | `{count} archivo modificado / {count} archivos modificados` | Mantiene la concordancia al insertarse tras «Copia y verifica». |
| `mirror.preview.copied.both` | Ambas formas: `{count} archivos (nuevos: {new}; cambiados: {changed})` | `one`: `{count} archivo (nuevos: {new}; modificados: {changed})`; `other`: `{count} archivos (nuevos: {new}; modificados: {changed})` | Unifica el término; la forma singular actual contiene «archivos», aunque el código nunca la selecciona. |
| `mirror.preview.changed` | `{count} cambiado / {count} cambiados` | `{count} modificado / {count} modificados` | Coherencia con los archivos que se van a actualizar. |
| `mirror.preview.shown.changed` | Cambiados | Modificados | Mismo término en el filtro. |
| `mirror.preview.reason.changed` | Cambiado en el origen | Modificado en el origen | Mismo término en la explicación de cada archivo. |
| `mirror.empty.what` | Un espejo mantiene un destino idéntico a su origen: los archivos nuevos y cambiados se copian y se verifican; los archivos borrados en el origen se archivan (o se borran) en el destino. En el origen no se escribe nada. | Un espejo mantiene un destino idéntico a su origen: los archivos nuevos y modificados se copian y se verifican; los archivos borrados en el origen se archivan (o se borran) en el destino. En el origen no se escribe nada. | Coherencia con la vista previa. |
| `mirror.editor.verifyNote` | Los archivos nuevos y cambiados siempre se verifican después de copiarlos. | Los archivos nuevos y modificados siempre se verifican después de copiarlos. | Mismo término en la ayuda del editor. |

En importación y exportación, evitaría participios aislados que parecen referirse al contenido variable:

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `export.done` | Exportado: `{what}`. | Exportación completada: `{what}`. | Funciona con una plantilla, varias plantillas, ajustes o una combinación. |
| `export.doneNamed` | Exportado: “`{name}`”. | Se ha exportado la plantilla “`{name}`”. | El referente siempre es una plantilla. |
| `import.done` | Importado: `{what}`. | Importación completada: `{what}`. | Evita «Importado: 1 plantilla de copia» sin necesitar concordancia dinámica. |
| `import.partly` | Importado: `{done}`. Error al guardar `{what}`: `{why}` | Importación completada: `{done}`. No se pudo guardar `{what}`: `{why}` | La primera frase se refiere solo a lo importado; la segunda identifica el fallo. |
| `import.failed` | Error al guardar `{what}`: `{why}` | No se pudo guardar `{what}`: `{why}` | Expresa claramente el resultado; el infinitivo evita problemas con el género y número de `{what}`. |
| `import.changed` | `{setting}`: cambiado | `{setting}`: se ha modificado | Actualmente `{setting}` es «Ignorar siempre al copiar»; «cambiado» carece de referente explícito. |
| `import.madeBy` | Hecho con Secopy `{theirs}`; este es `{ours}`. | Creado con Secopy `{theirs}`; estás usando Secopy `{ours}`. | «Este es» no identifica con claridad la versión en uso. |
| `import.problem.newer` | Necesita un Secopy más reciente: tiene ajustes que este no conoce (`{keys}`). | Requiere una versión más reciente de Secopy: contiene ajustes que esta versión no reconoce (`{keys}`). | «Un Secopy» y «este no conoce» son calcos poco naturales. |
| `errors.import.newer` | Este archivo se hizo con un Secopy más reciente (formato `{format}`). Actualiza Secopy para importarlo. | Este archivo se creó con una versión más reciente de Secopy (formato `{format}`). Actualiza Secopy para importarlo. | Forma natural de hablar de versiones de software. |
| `errors.save.newer` | `{file}` es de un Secopy más reciente: no se sobrescribe | `{file}` pertenece a una versión más reciente de Secopy: no se sobrescribe | Mismo criterio, conservando la garantía de no sobrescribir. |
| `queue.unsupported.newer` | Una tarea de un Secopy más reciente | Una tarea para una versión más reciente de Secopy | «Para» expresa mejor la compatibilidad requerida. |
| `queue.reason.newer` | Necesita un Secopy más reciente. | Requiere una versión más reciente de Secopy. | Coherencia con los demás mensajes de compatibilidad. |
| `import.notImported` | “`{setting}`” no se importa: es de un Secopy más reciente. | “`{setting}`” no se importa: pertenece a una versión más reciente de Secopy. | Evita el mismo calco. |

Las siguientes ayudas también necesitan una redacción más natural:

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `settings.systemCount.label`, `import.setting.systemCount` | Mostrar la cuenta de archivos ignorados | Mostrar el número de archivos ignorados | En España, «número» o «recuento» resulta más natural que «cuenta» aquí. |
| `settings.systemCount.help` | Cuántos archivos y directorios han dejado fuera “Ignorar siempre al copiar”, “Ignorar también” y los archivos de trabajo propios de Secopy. Los archivos ocultos se copian como cualquier otro. | Muestra el número de archivos y directorios excluidos por “Ignorar siempre al copiar” e “Ignorar también”, incluidos los archivos de trabajo de Secopy. Los archivos ocultos se copian como cualquier otro. | La frase actual hace que los archivos de trabajo parezcan agentes que excluyen otros archivos. |
| `settings.ignore.help` | Los archivos y directorios con estos nombres nunca se copian ni se reflejan, y un espejo nunca los elimina de su destino. * equivale a cualquier número de caracteres; ?, a uno. | Los archivos y directorios cuyos nombres coinciden con estos patrones se excluyen de las copias y los espejos. Los espejos tampoco los eliminan del destino. * representa cualquier número de caracteres; ?, uno. | Elimina «reflejar» y aclara que los nombres son patrones, no necesariamente coincidencias exactas. |
| `settings.checksumFile.help` | Un archivo `{file}` lista cada archivo copiado con su checksum, para poder verificar la copia más adelante, por ejemplo con `{command}`. | El archivo `{file}` incluye cada archivo copiado y su checksum, para poder verificar la copia más adelante, por ejemplo con `{command}`. | Evita el verbo «listar» y presenta naturalmente el nombre del archivo generado. |
| `settings.mhl.help` | ASC Media Hash List, la prueba de copia de la industria audiovisual (Silverstack, Hedge). Se escribe en un directorio ascmhl junto a los archivos; un historial existente se continúa y también lista los archivos que ya estaban. | ASC Media Hash List, el registro de verificación de copias de la industria audiovisual (Silverstack, Hedge). Se escribe en un directorio ascmhl junto a los archivos. Si ya existe un historial, se continúa y también se incluyen los archivos que ya estaban. | «Prueba de copia» es poco idiomático; la segunda oración actual es demasiado literal. |
| `settings.notify.help` | Solo cuando la ventana de Secopy no está delante. macOS pide permiso la primera vez. | Solo cuando la ventana de Secopy no está en primer plano. macOS pide permiso la primera vez. | «En primer plano» es el término preciso de interfaz. |
| `settings.menuBar.label`, `import.setting.menuBar` | Seguir con las tareas en la barra de menús al cerrar la ventana | Mantener las tareas en ejecución al cerrar la ventana | Las tareas continúan ejecutándose; la barra de menús ofrece el acceso y muestra su progreso. |
| `progress.resumeHelp` | Sigue desde donde se pausó (espacio). | Reanuda la tarea desde donde se puso en pausa (barra espaciadora). | «Se pausó» es menos natural y «espacio» no identifica bien la tecla. |
| `progress.pauseHelp.check` | Deja de leer hasta que reanudes (espacio). | Pausa la lectura hasta que reanudes la tarea (barra espaciadora). | Redacción más precisa y coherente con «Pausar». |
| `progress.pauseHelp.copy` | Deja de leer y escribir hasta que reanudes (espacio). | Pausa la lectura y la escritura hasta que reanudes la tarea (barra espaciadora). | Mismo criterio. |
| `queue.clearHelp` | Elimina todas las tareas de la cola, tras preguntar. | Pide confirmación antes de eliminar todas las tareas de la cola. | «Tras preguntar» resulta poco natural en una ayuda de botón. |
| `progress.cancelHelp.job` | Pregunta primero y luego cancela la tarea (⌘.). | Pide confirmación antes de cancelar la tarea (⌘.). | Expresión habitual y más directa. |
| `progress.cancelHelp.queue` | Pregunta primero y luego cancela esta tarea y detiene la cola (⌘.). | Pide confirmación antes de cancelar esta tarea y detener la cola (⌘.). | Mismo criterio. |

**C) Estilo, tipografía y longitud**

| Clave | Español actual | Propuesta | Motivo |
|---|---|---|---|
| `errors.check.listedIn` | `{why}` Listado en `{file}`. | `{why}`. Figura en `{file}`. | El código inserta errores sin punto final: actualmente puede aparecer «Es un directorio, no un archivo Listado en…». |
| `errors.check.listedInOnly` | Listado en `{file}`. | Figura en `{file}`. | Más natural y coherente con la entrada anterior. |
| `verify.found.listed` | `{count} archivo listado / {count} archivos listados` | `{count} archivo incluido / {count} archivos incluidos` | Evita «listado» como participio recurrente; la sección ya identifica los archivos de checksums. |
| `verify.found.notListed` | `{count} archivo sin listar / {count} archivos sin listar` | `{count} archivo no incluido / {count} archivos no incluidos` | «Sin listar» puede parecer una operación pendiente. |
| `verify.startHelp` | `Lee el archivo listado ({size})… / Lee los {count} archivos listados ({size})…` | `Lee el archivo incluido ({size}) y lo compara con su checksum; no se escribe nada. / Lee los {count} archivos incluidos ({size}) y compara cada uno con su checksum; no se escribe nada.` | Coherencia con los recuentos de la pantalla. |
| `format.percent`, `menubar.percent` | `{value} %` | `{value} %`, con espacio de no separación `U+00A0` | El espacio español ya es correcto; conviene impedir que el símbolo quede en otra línea. |
| `format.bytes.b`, `format.bytes.kb`, `format.bytes.mb`, `format.bytes.gb`, `format.bytes.tb`, `format.bytes.pb` | `{value} B`, `{value} KB`, etc. | Las mismas cadenas, con espacio de no separación entre cifra y unidad | Mantiene unido cada valor a su unidad. |
| `summary.openChecksumFile` | Abrir el archivo de checksums | Abrir archivo de checksums | Recorta un botón largo sin perder significado. |
| `mirror.switch.deleteNextRun` | Borrarlos en la próxima ejecución | Borrar en la próxima ejecución | Reduce una etiqueta larga; el diálogo ya establece qué archivos se borran. |
| `settings.ignore.restore` | Restaurar los valores por omisión | Restaurar valores por omisión | Acorta el botón manteniendo el término existente. |
| `progress.stop.alsoRemove` | Eliminar también los archivos ya copiados | Eliminar también los archivos copiados | «Ya» es redundante en esta opción del diálogo de cancelación. |
| `copy.title`, `summary.newCopy` | Copia nueva | Nueva copia | Unifica el patrón con «Nueva plantilla» y «Nuevo espejo». |
| `errors.field.ignoreInGlobal` | “`{pattern}`” ya está en Ajustes › Ignorar siempre al copiar. | “`{pattern}`” ya está en Ajustes › “Ignorar siempre al copiar”. | Falta marcar el nombre literal de la opción, como exige la guía. |
| `settings.ignore.inGlobal` | Ya está en Ajustes › Ignorar siempre al copiar. | Ya está en Ajustes › “Ignorar siempre al copiar”. | Mismo criterio. |
| `copy.alsoIgnoreHint` | Nombres que esta copia (y su plantilla) nunca copia, además de Ajustes › Ignorar siempre al copiar. | Nombres excluidos de esta copia y de su plantilla, además de los definidos en Ajustes › “Ignorar siempre al copiar”. | Mejora la redacción y marca la opción citada. |
| `presets.editor.alsoIgnoreHint` | Nombres que esta plantilla nunca copia, además de Ajustes › Ignorar siempre al copiar. | Nombres excluidos de las copias que usan esta plantilla, además de los definidos en Ajustes › “Ignorar siempre al copiar”. | La plantilla configura la copia; no copia por sí misma. |
| `mirror.editor.alsoIgnoreHint` | Nombres que este espejo nunca copia ni elimina, además de Ajustes › Ignorar siempre al copiar. | Nombres que este espejo nunca copia ni elimina, además de los definidos en Ajustes › “Ignorar siempre al copiar”. | Falta el vínculo con los nombres definidos y las comillas de la opción. |
| `copy.aboutVerifyingText` | Copia y verificación vuelve a leer cada archivo del destino y lo compara con el checksum del origen; si no coincide, se vuelve a copiar una vez. Copia es más rápida, pero no vuelve a leer las copias. | “Copia y verificación” vuelve a leer cada archivo del destino y lo compara con el checksum del origen; si no coincide, se vuelve a copiar una vez. “Copia” es más rápida, pero no vuelve a leer las copias. | Las comillas distinguen los nombres de los modos de los sustantivos comunes. |
| `presets.empty.what` | Una plantilla de copia guarda un origen y sus opciones (el directorio en sí o solo su contenido, tipos de archivo, Ignorar también): elígela en Copia nueva para preparar una copia frecuente de una vez. | Una plantilla de copia guarda un origen y sus opciones (el propio directorio o solo su contenido, los tipos de archivo e “Ignorar también”). Elígela en “Nueva copia” para preparar una copia habitual en un solo paso. | Marca los nombres de UI y elimina «de una vez», que aquí resulta impreciso. |
| `presets.empty.how` | Crea una con “+ Nueva plantilla…”, o con “Guardar como…” en Copia nueva. | Crea una con “+ Nueva plantilla…” o con “Guardar como…” en “Nueva copia”. | Elimina una coma innecesaria y marca el nombre de la pantalla. |

Las propuestas de longitud son editoriales: **no he comprobado recortes ni saltos de línea en la app renderizada**. No acortaría «Cancelar tarea y salir» ni «Abrir archivo de checksums» hasta volverlos ambiguos.

**Composiciones revisadas que pueden mantenerse**

- `{kept}` funciona en «No se copió nada: 1 archivo distinto sin cambios» y con varios archivos.
- `{copied}` y `{removed}` concuerdan correctamente con «Copia y verifica», «Archiva» y «Borra».
- En `mirror.preview.copied.both`, el código exige archivos nuevos **y** modificados: `{count}` siempre es al menos dos. El singular incorrecto señalado arriba es una forma inalcanzable actualmente.
- `{days}` ya incluye «día/días». Las cadenas que lo reciben no añaden otra unidad ni fuerzan un número incorrecto.
- `{what}` en importación y exportación puede mezclar géneros y números. Las propuestas «Importación completada:…» y «Exportación completada:…» evitan depender de ellos.
- «Conservar ambas» es correcto para **plantillas**; «Conservar ambos», para **archivos**. No deben unificarse.
- «Cancelar», «Hecho», «Archivo», «Edición», «Visualización», «Ventana» y «Mostrar en el Finder» pueden mantenerse. Apple también emplea «Mostrar en el Finder» en su documentación española. [Referencia de Apple](https://support.apple.com/es-es/guide/logicpro/lgcp4492eed9/mac).

**Elecciones terminológicas que revisaría**

| Término | Recomendación |
|---|---|
| **Paranoica** | Preferiría **Exhaustiva** para el nivel de comparación: informa mejor de lo que hace. «Paranoica» concuerda correctamente con «comparación», pero su tono es poco propio de macOS. Sería una decisión terminológica, no una corrección obligatoria. |
| **Espejo** | Lo mantendría como término del producto. Evitaría el verbo **reflejar**; usaría «actualizar el espejo» o «ejecutar el espejo» según la acción. |
| **Cambiado** | Usaría **modificado** para archivos; reservaría «cambiar» para ajustes, destinos y decisiones del usuario. |
| **Listar** | Preferiría **incluir**, **figurar** o **registrar**, según se trate de pertenecer a una lista o escribir una entrada en un historial. |
| **Archivo fallido** | Es comprensible, pero «error al copiar/verificar el archivo» suele ser más preciso. No lo cambiaría globalmente sin distinguir la operación. |
| **Valores por omisión** | Puede mantenerse: es una expresión válida y presente en la documentación de Apple. No hay motivo para introducir «predeterminados» como segundo término. [Ejemplo de Apple](https://support.apple.com/es-es/guide/motion/motnad41607e/6.4/mac/26.6). |
| **Los archivados** | Mantendría la elección del usuario para el conjunto. En explicaciones extensas, «los archivos archivados» puede ayudar, sin convertir **archivo** en nombre del depósito. |
| **Plantilla, directorio, checksum, tarea, Ajustes; borrar/eliminar** | Mantendría todas estas decisiones. La distinción entre **borrar** y **eliminar** se respeta en general y aporta precisión al flujo. |

Una observación adicional sobre la guía: «una cadena nunca es un fragmento de otra frase» no describe completamente el código actual, que compone `{parts}`, `{copied}`, `{removed}` y `{what}`. Conviene documentarlos como **listas o grupos nominales completos**, e indicar sus referentes y reglas de concordancia.