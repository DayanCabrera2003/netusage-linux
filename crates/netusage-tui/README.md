# netusage-tui

Interfaz de terminal de netusage: muestra el consumo de red total y por
aplicación, con conmutador de periodo (hoy / semana / mes / mes anterior), al
estilo de la pantalla "Uso de datos" de Android.

## Cómo arrancar

Requiere el demonio (`netusaged`) corriendo y persistiendo en una base SQLite.

```sh
# Contra la base por defecto del servicio:
netusage-tui

# Contra una base concreta (p. ej. la de una prueba):
netusage-tui --db /tmp/netusage.db --period week --refresh-secs 2
```

La TUI abre la base en **solo lectura** (no necesita privilegios; la base es
`0644`). No usa el socket IPC: lee directamente la base, que es el camino
primario de la Fase 4.

## Atajos de teclado

| Tecla | Acción |
|-------|--------|
| `q` / `Esc` | Salir (o cerrar el detalle si está abierto) |
| `Tab` / `l` / `→` | Periodo siguiente |
| `h` / `←` | Periodo anterior |
| `j` / `↓` | Bajar en la lista |
| `k` / `↑` | Subir en la lista |
| `Enter` | Abrir/cerrar el detalle de la app |
| `r` | Refrescar ahora |

## Arquitectura por capas

- **Datos** (`data.rs`) y **modelo** (`model.rs`): sin dependencias de ratatui,
  reutilizables por una futura GUI. `data.rs` es la única frontera con la capa de
  persistencia.
- **Estado** (`state.rs`) y **reductor** (`update.rs`): transiciones puras,
  testeables sin terminal.
- **Widgets** (`ui/`): funciones de render puras sobre `&AppState`, testeadas con
  el `TestBackend` de ratatui.
- **Bucle** (`app.rs`, `event.rs`): orquestación síncrona (poll de crossterm con
  timeout = intervalo de refresco) y mapeo tecla → mensaje.
