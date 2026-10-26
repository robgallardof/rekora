# Rekora

Memoria local, verificable y portable para agentes de IA que programan. *(English: [README.md](README.md))*

> **Estado: desarrollo temprano (rebanada vertical de la etapa B).** Sin release ni publicación. Todo lo
> de abajo se ejecutó en Windows 11; Linux/macOS corren en CI pero aún no se han probado a mano. El
> detalle de qué está hecho y qué no está en [docs/STATUS.md](docs/STATUS.md).

Los agentes olvidan las decisiones entre sesiones y un segundo agente empieza de cero. Rekora guarda,
por proyecto, decisiones, restricciones, hechos y problemas conocidos — cada uno con sus fuentes,
estado e historial — y le entrega al agente solo lo relevante para la tarea, dentro de un presupuesto
de tokens, o responde `no_evidence` cuando no tiene nada.

- Un solo ejecutable, offline, sin modelo, sin daemon, sin telemetría.
- Una base SQLite por proyecto en `.rekora/` (ignorada por Git automáticamente).
- Las memorias citan archivos; si un archivo citado cambia, el resultado se marca **possibly stale**.
- Lo que escribe un agente queda como `candidate` hasta que una persona lo verifica.
- CLI y servidor MCP por stdio.

## Compilar

Requiere Rust estable (MSRV declarado 1.88, aún sin verificar en CI). SQLite va incluido; la primera compilación necesita compilador de C.

```bash
cargo build --release
```

## Inicio rápido

```bash
cd tu-repo
rekora init
rekora memory add --type decision --source docs/adr/0001.md:3 --verified "El almacén local usa SQLite en modo WAL"
rekora memory search "almacén sqlite" --explain
rekora context --task "trabajar en la capa de almacenamiento" --budget 1024
rekora doctor
```

Todos los comandos aceptan `--format json`. Los errores en JSON son
`{"error": {"code": "...", "message": "..."}}` con códigos estables. Códigos de salida: `0` ok (una
búsqueda `no_evidence` es ok), `1` error, `2` uso incorrecto, `3` `doctor` encontró un problema crítico.

La tabla completa de comandos está en el [README en inglés](README.md#commands-implemented).

## Desde un agente (MCP)

```bash
rekora mcp serve --stdio
```

Herramientas: `search`, `context`, `remember`, `status`. **Aún no probado dentro de Claude Code, Codex
u otro host real**; está probado con un cliente MCP JSON-RPC real por stdio (protocolo `2025-11-25`).
Los adaptadores de host (`rekora agent install ...`) están planeados, no implementados.

## Limitaciones actuales

- Solo búsqueda por palabras clave (FTS5); sin sinónimos ni embeddings.
- Solo ámbito de proyecto; sin import/export todavía.
- Sin handoffs/tareas, sin indexado consciente de ramas/worktrees, sin adaptadores de host.
- La redacción de secretos es por patrones y puede fallar con formatos raros.
- Los tokens son estimaciones conservadoras (bytes UTF-8 / 3).
- No se han ejecutado benchmarks; no se hacen afirmaciones de rendimiento.

## Licencia

[MIT](LICENSE).
