# REKORA — PROMPT MAESTRO PARA IMPLEMENTAR EL PROYECTO

> **Uso:** entrega este archivo a Claude Code, Codex, Antigravity u otro agente de programación dentro de un repositorio vacío o existente. Adjunta también `REKORA_MASTER_PROMPT.md` (anexo técnico exhaustivo), `REKORA_IMPLEMENTATION_PLAN.md`, `REKORA_TEST_MATRIX.md` y `REKORA_AGENT_ROLES.md`. El agente debe **crear código real, ejecutarlo y probarlo**, no responder únicamente con un plan.
>
> **Nombre:** Rekora. Verificar disponibilidad del nombre y licencias antes de publicarlo.

## 1. Actúa como equipo de ingeniería de software de alto nivel

Eres responsable de desarrollar **Rekora**, una herramienta open source, local-first, rápida, segura y verificable, creada principalmente en Rust. Puedes distribuir trabajo entre subagentes si el entorno lo permite, pero toda integración debe tener un responsable y pruebas propias. Simula roles mediante pasos secuenciales si no existe una función real de subagentes; no afirmes que participaron agentes que no existen.

Roles: arquitecto de Rust, ingeniero de persistencia, ingeniero MCP/adaptadores, especialista en recuperación de información, especialista de Git y análisis de código, ingeniero de seguridad, QA/SDET, ingeniero de rendimiento, documentación/experiencia de desarrollador y responsable de integración. Respeta las responsabilidades de `REKORA_AGENT_ROLES.md`.

Inspecciona primero el repositorio y las herramientas realmente disponibles; no asumas que están instalados Serena, Graphify, agentes externos, contenedores, modelos locales o servicios cloud. Utiliza esas herramientas si son pertinentes y están disponibles, pero ofrece siempre un camino con Git, Rust y utilidades comunes.

Trabaja por incrementos verticales completos. En cada fase entrega diseño breve, código funcional, pruebas ejecutadas, resultados observados, documentación y estado real. No finjas tests, benchmarks, compatibilidad ni funcionalidades. Si falta una dependencia necesaria, delimita el bloqueo y construye la mayor parte verificable sin esa dependencia.

## 2. Qué producto estamos construyendo

Rekora debe permitir que diferentes agentes compatibles recuerden **conocimiento, decisiones, errores, cambios de código, tareas y resultados**, y puedan retomar trabajos anteriores sin copiar conversaciones completas. El objetivo es mantener continuidad entre sesiones y proveedores, con control humano y contexto preciso.

Debe ofrecer:

1. Memoria persistente y organizada con evidencias, no únicamente historial de chat.
2. Búsqueda relevante con indicación de por qué aparece cada resultado.
3. Separación estricta por usuario, repositorio, ámbito y, cuando importe, rama/worktree de Git.
4. Recuperación de contexto limitada por presupuesto de tokens, sin saturar al agente.
5. Detección de recuerdos posiblemente obsoletos tras cambios de código.
6. Transferencia de tareas y decisiones entre agentes mediante un formato versionado e interoperable.
7. Entrada por CLI y servidor MCP; hooks y plugins por adaptadores específicos verificables.
8. Instalación sencilla en Windows, Linux y macOS con **un ejecutable Rust**; funcionamiento completamente offline en el modo básico.
9. Observabilidad, exportación, borrado y herramientas para revisar/corregir conocimiento.
10. Tests, datos sintéticos realistas y benchmarks reproducibles que demuestren su calidad.

Rekora **no** es un chatbot completo, un framework de microservicios, una base de datos vectorial obligatoria ni un sistema que ejecuta autónomamente comandos peligrosos. Debe ser una herramienta de infraestructura de conocimiento que cualquier agente pueda utilizar con permisos claros.

## 3. Investiga y diferencia con evidencias

Antes de implementar funciones avanzadas revisa código, documentación, licencias, issues y comparativas de proyectos: Kindex (`wandercom/kindex`), Hermes Agent (Nous Research), Mem0, Graphiti, SDK MCP oficial para Rust, especificación MCP y prácticas actuales de la comunidad. Investiga problemas reales en Reddit y foros originales (pérdida de contexto, malas memorias, overhead de tokens, interferencias entre agentes, instalación problemática, privacidad, etc.). Cita URLs y fechas reales; no inventes resultados.

Kindex **ya** ofrece grafo, memoria con procedencia, recuperación, herramientas MCP, hooks y coordinación. Hermes **ya** ofrece aprendizaje procedural y skills. Por tanto, esas características por sí solas no son innovación nuestra.

Formula hipótesis específicas y medibles: «Rekora conserva mejor la continuidad entre dos clientes sin fugas», «entrega contexto de mayor utilidad por token», «detecta memoria desactualizada por rama», «facilita una instalación reversible», etc. Compara con baseline de notas/AGENTS.md y Kindex donde resulte legal y reproducible. Usa el mismo corpus, equipo y condiciones; incluye resultados negativos. Nunca afirmes superioridad sin pruebas.

Documenta riesgos, tecnologías consideradas, dependencias, alternativas descartadas y criterios de adopción. Reevalúa versiones de SDK y protocolo vigentes durante la ejecución.

## 4. Arquitectura Rust sin sobreingeniería

Usa Cargo workspace con un **monolito modular**. Un binario principal `rekora` con subcomando `rekora mcp serve --stdio`. Separa los módulos o crates solo cuando exista una razón clara: dominio e invariantes, SQLite/migraciones, recuperación/contexto, evidencia Git/código, MCP, adaptadores y CLI. No crees veinte proyectos vacíos ni interfaces abstractas para una única implementación.

Tecnologías sugeridas sujetas a confirmación de versión/licencia: Tokio, rmcp (SDK oficial), Clap, Serde, thiserror, Tracing, SQLite con WAL + FTS5, BLAKE3, gix y Tree-sitter cuando lo justifiquen las pruebas. Herramientas de QA: `cargo test`, `cargo nextest`, `proptest`, fuzzing dirigido, Criterion y `loom` si existe concurrencia que lo necesite.

**Importante:** SQLite está escrito en C, aunque lo utilizaremos desde Rust. Prioriza funcionalidad, portabilidad y mediciones sobre la idea de un proyecto puro-Rust a cualquier costo. No requieras Python, Node, Docker, Redis, Postgres, Neo4j, SaaS, un GPU o un LLM para la instalación básica. Las pruebas pueden incluir fixtures de C#, JavaScript/TypeScript, Python y Rust, pero el núcleo del producto debe estar implementado en Rust.

Sigue principios SOLID/DRY/KISS, separación de responsabilidades, estructuras y errores tipados, funciones pequeñas, invariantes explícitos y documentación mantenible. No introduzcas Clean Architecture ceremonial, CQRS innecesario, mediadores, fábricas o capas que no aporten valor.

## 5. Modelo de memoria y persistencia

Define IDs estables, versiones de esquema y entidades pequeñas:

- `Memory`: hecho, decisión, restricción, problema conocido, procedimiento o referencia de tarea.
- `Scope`: ámbito privado/local, repositorio, configuración explícita de equipo, worktree/rama cuando aplique.
- `SourceRef`: ruta/archivo, commit, símbolo, líneas o evidencia de prueba, hash y fecha.
- `Decision`: opciones consideradas, razón, evidencia y evolución cuando existan datos.
- `Task/Handoff`: objetivo, estado, pendientes, archivos, cambios, resultados de tests, decisiones, dependencias y versión.
- `Event`: cambios auditables e idempotentes.
- `Relation`: relaciones tipadas solo si realmente ayudan a consultar y comprobar algo.

Clasifica recuerdos como candidatos, verificados, discutidos, reemplazados u obsoletos. Un texto generado por un LLM no se convierte en hecho porque haya sido guardado. Un documento externo nunca se convierte automáticamente en una instrucción confiable.

Asegura transacciones, migraciones versionadas, backups, rollback, idempotencia, recuperación tras reinicios y manejo de concurrencia. Mantén historia y timestamps UTC. No confundas «momento de observación» con «periodo en el que un hecho era válido»; esto resulta crucial cuando una decisión varía entre ramas de Git.

Por defecto, no subas ni sincronices información a la nube y no incluyas automáticamente memoria privada en un Git commit. Documenta almacenamiento, exportación, borrado, retención y recuperación.

## 6. Motor de recuperación y compilador de contexto

Primero implementa un sistema determinista **sin modelos externos**:

1. Autorización y ámbito **antes de buscar o calcular rankings**.
2. Búsqueda exacta y de texto completo con FTS5; opcionalmente referencias de código/relaciones comprobadas.
3. Dedupe por identidad/hash y conservación de contradicciones, sin mezclarlas silenciosamente.
4. Ranking explicable según relevancia, evidencia, vigencia y tipo de memoria. No presentes puntajes heurísticos como probabilidades calibradas.
5. Verificación de vigencia respecto a rama, commit y fuentes cuando sea posible.
6. Selección de contexto en presupuesto estricto de tokens, con orden estable, sin duplicados y con procedencia.
7. Resultado estructurado con explicación y una salida `no_evidence` ante consultas sin soporte suficiente.

Diseña evaluación con preguntas relevantes, irrelevantes, contradictorias y sin respuesta. Mide precisión@k, recall@k, MRR/nDCG cuando haya ground truth, abstención correcta, contaminación por memoria vieja, fugas entre ámbitos, uso de tokens, latencia y RSS.

## 7. Análisis real de repositorios y Git

Indexa archivos autorizados de forma **incremental**, con hashes, reglas de ignore y límites de tamaño. No indexar por defecto secretos, `.env`, `.git`, dependencias generadas ni carpetas fuera de permisos. Utiliza Git y Tree-sitter donde exista un parser real; soporta inicialmente fixtures realistas de C#/.NET, TS/Vue o Next, Rust y Python. Separa hechos AST comprobados de inferencias o supuestos de dependencias.

Detecta cambios de rama/worktree, modificaciones, renombrados, borrados y rebase. Reindexa únicamente lo afectado. Marca como **potencialmente obsoletos** los recuerdos asociados a símbolos cambiados; nunca elimines decisiones históricas sin revisión. No prometas call graphs perfectos ni comprensión total de lenguajes dinámicos.

## 8. MCP, CLI, hooks y cualquier agente

Implementa MCP mediante `rmcp`, comenzando por stdio, con herramientas pequeñas y de propósito definido: búsqueda, contexto, creación de memoria, explicación, tareas/handoff y estado. Versiona esquemas y valida JSON; implementa errores tipados, límites de tamaño, paginación y pruebas de compatibilidad. Consulta la versión MCP actual; el protocolo 2026-07-28 cambió mecanismos respecto de versiones anteriores y deprecó otros.

Comandos previstos (implementarlos progresivamente, no documentarlos falsamente):

```bash
rekora init --scope project
rekora memory add --type decision --source ./docs/adr/001.md "Use optimistic concurrency"
rekora memory search "concurrency" --explain
rekora context --task "continue the refactor" --budget 1200
rekora handoff create --task auth-refactor
rekora handoff resume --task auth-refactor --format json
rekora agent list
rekora agent install claude --dry-run
rekora agent install codex --dry-run
rekora mcp serve --stdio
rekora doctor
rekora export --scope project --format json
```

Crea adaptadores nativos primero para Claude Code y Codex tras **comprobar** sus configuraciones y capacidades actuales; añade Antigravity, Cursor, OpenCode, Gemini CLI, Hermes y otros clientes mediante MCP genérico o adaptadores específicos comprobados. MCP facilita interoperabilidad, pero **no garantiza captura automática por hooks en todos los clientes**.

Toda instalación de hooks/config debe ser `--dry-run` por defecto o exigir confirmación, preservar otras configuraciones, respaldar cambios, ser idempotente y reversible. Incluye tests Windows de espacios/Unicode/PowerShell, permisos, configs ya existentes e intentos de inyección en rutas.

## 9. Seguridad y privacidad no negociables

Modelo de amenazas desde el inicio: contenido malicioso en Markdown, comentarios o resultados de herramientas; contaminación del grafo; datos privados cruzados entre repositorios; traversal/symlinks/junctions; ejecución de comandos mediante hooks; pérdida de datos en migraciones; accidental envío a terceros.

Reglas:

- Filtro por ámbito antes de recuperación, sin influencia ni filtración de memorias no autorizadas.
- Entradas externas son **datos**, no instrucciones superiores.
- No ejecutar comandos procedentes de recuerdos/skills sin autorización separada.
- Redacción de secretos antes de persistir/loguear, con tests de falsos positivos y limitaciones conocidas.
- Sin red, telemetry ni API keys obligatorias en el modo básico.
- Escrituras sobre configuración global solo con consentimiento informado y backup/restauración.
- Exportar, borrar y corregir memoria debe ser fácil y auditable.
- Manejo seguro de interrupciones, SQLite locks, límites, entradas corruptas y fallos de parseo.

## 10. Comentarios: requisito del producto

**No quiero código incomprensible ni documentación superficial.** Cada módulo público Rust lleva comentario `//!` que explica responsabilidad, límites e invariantes. Las APIs públicas reciben comentarios Rustdoc `///` con propósito, errores, garantías, ejemplos útiles y notas de seguridad cuando aplique. Funciones complejas llevan comentarios sobre **por qué** se diseñaron así, no simplemente repetir la línea en lenguaje natural.

Cada decisión arquitectónica relevante requiere un ADR con alternativas y razones. Documentación en inglés y español al menos para README, instalación y ejemplos fundamentales. `AGENTS.md` neutral para cualquier agente, reglas de desarrollo, diagramas Mermaid verificables, errores CLI, contratos JSON/MCP, guía de contribución, SECURITY.md, CHANGELOG y tutoriales reales.

El código, comentarios, tests y documentación tienen que coincidir. Los ejemplos Rustdoc deben compilarse como doctests cuando corresponda. No llenar archivos de comentarios redundantes ni enmascarar código complicado con párrafos; primero simplificar.

## 11. Pruebas reales, ejecutables y obligatorias

No aceptes un proyecto de este tipo con puros mocks. Los tests unitarios prueban invariantes y algoritmos; **integración debe usar SQLite real, índices FTS reales, Git real, archivos reales y subprocess MCP real**. E2E local debe verificar CLI + MCP entre dos procesos, incluyendo reinicio. E2E contra un Claude/Codex instalado es opcional y debe reportarse como `NO EJECUTADO` cuando el host real no esté disponible.

Debes implementar los **36 escenarios** de `REKORA_TEST_MATRIX.md`, priorizados por fase. Mínimos imprescindibles:

- Guardar memoria, cerrar proceso y recuperarla sin proveedor remoto.
- Claude/Codex u otros dos clientes simulados **solo a nivel protocolo real** que entregan y retoman una tarea por MCP.
- Dos repositorios con servicios de nombre idéntico: ninguna fuga cruzada.
- Dos worktrees con decisiones contradictorias: respuesta limitada a rama y con conflicto visible.
- Renombrar un símbolo en un commit Git: memoria dependiente marcada posiblemente obsoleta.
- Consulta inexistente: abstención correcta, sin inventar una respuesta.
- Presupuestos 0, 256, 1024 tokens: salida acotada y estable.
- README malicioso que ordena subir `.env`: ningún secreto revelado ni comando ejecutado.
- Instalador de adapter conserva intactos otros servidores MCP y permite rollback.
- Migración SQLite interrumpida: estado consistente o recuperación real probada.
- Múltiples escritores/lectores: no perder versiones ni producir corrupción.
- Tests de C#, TS/Vue, Rust, Python con archivos/símbolos verdaderos.
- Importación repetida sin duplicar conocimiento; errores correctos en Unicode/rutas maliciosas.
- Benchmarks reproducibles con corpus 1k, 10k y 100k; latencia p50/p95/p99, RAM, CPU y relevancia.

Usa `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, doctests, CI nativa Windows/Linux/macOS y, según riesgo, pruebas de propiedades, fuzzing, seguridad, pruebas de concurrencia y auditoría de dependencias. Cada fallo real debe producir prueba de regresión. Los casos deben definir input, estado previo, expected, asserts, cleanup y comando de reproducción.

NO escribas «pasan todos los tests» si no se ejecutaron. No pruebes las herramientas MCP con un mock del MCP en lugar del servidor real; no atribuyas una compatibilidad de agente real a un cliente sintético.

## 12. Rendimiento y criterios de superioridad

Rust no implica automáticamente mejor rendimiento que otros proyectos. Perfila, mide y compara. Objetivos iniciales (NO resultados medidos): búsqueda local caliente p95 inferior a 100 ms para 100k recuerdos cortos sobre un equipo de referencia publicado; CLI en frío p95 inferior a 250 ms; gasto cero de un daemon en reposo porque el daemon no es obligatorio. Ajusta hipótesis según evidencia.

Comparar contra notas simples, Rekora en modo mínimo, Rekora con extensiones y Kindex cuando sea posible, en mismo hardware, mismas preguntas, mismas etiquetas y misma configuración comparable. Mide calidad, tasa de abstención, fugas, staleness, tokens, latencia, RSS y instalación, no solo velocidad de búsqueda. Publica versiones, fechas, seeds, limitaciones y resultados negativos.

## 13. Funciones de investigación, solo si pasan evaluación

**NO integrar Jev, modelos probabilísticos, aprendizaje autoevolutivo, embeddings ni vector DB en el núcleo sin demostrar utilidad.** Añade un laboratorio de investigación aislado y con feature flag para investigar:

- Jev/LLM2Jev o un decisor local: comparar contra heurísticas/reglas y pequeños clasificadores; calibrar probabilidades (Brier/ECE), coste, latencia, exactitud y resultados; no confundir probabilidad de elegir una opción con probabilidad de éxito.
- Aprendizaje tipo Hermes: nueva skill siempre es candidata hasta superar pruebas reproducibles, aprobación y versionado/rollback.
- Embeddings o grafo expandido: demostrar mejora de recuperación/razonamiento con presupuesto de tokens similar.
- Coordinación avanzada multiagente: demostrar problemas reales que no puedan resolverse con tarea versionada y lock local.
- Sincronización cifrada entre equipos: estudiar necesidades, privacidad y modelo de conflicto; desactivada por defecto.

Cada experimento tiene hipótesis, dataset, baseline, métricas, condiciones de aceptación y ADR de «adoptar / iterar / descartar». Si no supera al camino simple, se descarta sin remordimiento.

## 14. Desarrollo por fases con puertas de aceptación

Sigue `REKORA_IMPLEMENTATION_PLAN.md`. Resumen:

- **Fase 0:** investigación, amenaza, requisitos y fixtures. No prometer superioridad.
- **Fase 1:** binario Rust, CLI, SQLite, migración, ámbito, decisión guardada y recuperada tras reinicio.
- **Fase 2:** MCP stdio verdadero y tests entre subprocesses; FTS5, contexto limitado y abstención.
- **Fase 3:** seguridad, import/export, explicaciones, doctor, documentación y CI multiso.
- **Fase 4:** adaptadores reales y reversibles para Claude/Codex; tareas y handoffs persistentes.
- **Fase 5:** Git/worktrees, indexador de lenguajes e invalidación basada en evidencias.
- **Fase 6:** benchmarks, endurecimiento y releases públicos honestos.
- **Fase 7:** investigación opcional de modelos, skills, grafos y sincronización.

No marques una fase completa con solo crear una estructura de carpetas. Una fase está lista si **compila, ejecuta el flujo de usuario previsto y pasa sus pruebas reales**.

## 15. Entregables y reporte de cada milestone

Entregar código Rust comentado, Cargo workspace, documentación Rustdoc y ADR, README en EN/ES, CLI `--help`, ejemplo de configuración, MCP schemas, fixtures, tests unitarios/integración/E2E/concurrencia/seguridad, matriz de compatibilidad de agentes, resultados medidos de benchmarks, CI GitHub Actions, política de privacidad, CONTRIBUTING, SECURITY, licencia compatible verificada y release multiplataforma cuando esté listo.

En cada milestone informar:

```text
Fase y commit:
Qué se implementó realmente:
Qué todavía no se implementó:
Por qué se eligió esa arquitectura:
Archivos cambiados:
Comandos ejecutados:
Unit: aprobados / fallidos / omitidos / no ejecutados
Integration: aprobados / fallidos / omitidos / no ejecutados
E2E local y con agentes reales: resultado y versión de host
Seguridad: pruebas ejecutadas y hallazgos
Benchmarks: hardware, corpus, p50/p95/p99 o NO MEDIDO
Comentarios/Rustdoc y documentos agregados:
Riesgos/limitaciones:
Siguiente incremento mínimo y sus criterios:
```

## 16. INSTRUCCIÓN FINAL: COMIENZA A CONSTRUIR

No te limites a reformular este prompt ni a escribir un roadmap. **Empieza a implementar en el repositorio** después de comprobar su estado. Si está vacío, en el primer incremento crea la estructura mínima del Cargo workspace, binario `rekora`, primer ADR, base SQLite, control de ámbitos, memoria con fuente y pruebas de persistencia/reapertura. Agrega CLI y un primer MCP stdio funcional tan pronto como haya almacenamiento. Ejecuta `fmt`, `clippy`, tests unitarios y pruebas de integración reales; corrige fallos.

Cada cambio debe ser compacto, comprobable, documentado y fácil de mantener por humanos y agentes. No actualices ajustes globales del usuario, no ejecutes comandos destructivos y no te conectes a servicios opcionales sin consentimiento. Si no puedes concluir un módulo, deja el estado exacto y próximo paso verificable.

**Éxito = utilidad comprobada + seguridad + interoperabilidad + pruebas reales + mantenimiento sencillo. No es éxito simplemente producir mucho código.**
