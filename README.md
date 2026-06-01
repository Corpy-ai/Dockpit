# Docker Manager v3.0 - Rust Edition

## 🚀 Sobre el Proyecto

Docker Manager v3.0 es una completa reescritura en Rust del gestor de contenedores Docker, eliminando completamente los problemas de renderizado del script Bash v2.2 y ofreciendo un rendimiento excepcional.

### Ventajas sobre la versión Bash

- **Interfaz perfecta**: Sin problemas de renderizado, usando ratatui para TUI nativa
- **Rendimiento 10x superior**: Compilado a código nativo, sin overhead de interpretación
- **Manejo asíncrono**: Todas las operaciones son no-bloqueantes
- **Gestión de memoria eficiente**: Sin fugas de memoria ni buffers limitados
- **Portabilidad**: Un único ejecutable sin dependencias de sistema

## 📋 Características

### Interfaz TUI Interactiva
- Panel dual: lista de contenedores + logs/estadísticas
- Navegación fluida con teclado
- Actualización en tiempo real sin parpadeos
- Modo expandido para logs a pantalla completa
- Indicadores visuales de estado de contenedores

### Operaciones Docker
- ✅ Listar todos los contenedores
- ✅ Iniciar/Detener/Reiniciar contenedores
- ✅ Pausar/Reanudar contenedores
- ✅ Eliminar contenedores
- ✅ Ver logs en tiempo real
- ✅ Monitorear estadísticas (CPU, memoria, red, disco)
- ✅ Ejecutar comandos dentro de contenedores
- ✅ Inspeccionar configuración de contenedores

### Funciones Avanzadas
- 📋 Copiar logs al portapapeles (múltiples opciones)
- 🔢 Navegación numérica rápida (1-9)
- 🔍 Scroll en logs con Page Up/Down
- 💾 Guarda último contenedor seleccionado
- 🎨 Colores e iconos para mejor visualización

## 🛠️ Instalación

### Opción 1: Usar el ejecutable precompilado

```bash
# Copiar el ejecutable al sistema
sudo cp target/release/docker-manager /usr/local/bin/
sudo chmod +x /usr/local/bin/docker-manager

# Ejecutar
docker-manager
```

### Opción 2: Compilar desde fuente

```bash
# Instalar Rust si no lo tienes
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Compilar el proyecto
cargo build --release

# El ejecutable estará en target/release/docker-manager
./target/release/docker-manager
```

## 📖 Uso

### Modo TUI Interactivo (por defecto)
```bash
docker-manager
```

### Comandos CLI
```bash
# Listar contenedores
docker-manager list

# Iniciar un contenedor
docker-manager start <container_name>

# Detener un contenedor
docker-manager stop <container_name>

# Reiniciar un contenedor
docker-manager restart <container_name>

# Ver logs
docker-manager logs <container_name> --lines 100
docker-manager logs <container_name> --follow

# Ver estadísticas
docker-manager stats <container_name>

# Ejecutar comando en contenedor
docker-manager exec <container_name> ls -la
```

## ⌨️ Atajos de Teclado (Modo TUI)

### Navegación
- `↑/↓` o `j/k` - Navegar entre contenedores o scroll en logs
- `←/→` o `h/l` - Cambiar entre panel de contenedores y logs
- `1-9` - Saltar directamente al contenedor N
- `n` - Entrada numérica para saltar a contenedor específico

### Vistas
- `L` - Cambiar a vista de Logs
- `S` - Cambiar a vista de Estadísticas
- `F` - Alternar modo de logs expandido

### Operaciones
- `D` - Abrir menú de operaciones Docker
- `C` - Abrir menú del portapapeles
- `R` - Reiniciar contenedor seleccionado
- `Q` - Salir de la aplicación

### Scroll en Logs
- `Page Up` - Subir 10 líneas
- `Page Down` - Bajar 10 líneas
- `Home` - Ir al inicio
- `End` - Ir al final

## 🔧 Requisitos del Sistema

- Docker instalado y en ejecución
- Usuario con permisos para Docker (grupo docker)
- Terminal con soporte para 256 colores
- Opcional: xclip/xsel para soporte de portapapeles en Linux

## 🐛 Solución de Problemas

### "Failed to connect to Docker daemon"
```bash
# Asegúrate de que Docker está en ejecución
sudo systemctl start docker

# Añade tu usuario al grupo docker
sudo usermod -aG docker $USER
# Luego cierra sesión y vuelve a entrar
```

### Problemas con el portapapeles
```bash
# Instalar herramienta de portapapeles en Linux (sesión gráfica local)
sudo apt-get install xclip  # Debian/Ubuntu  (o wl-clipboard en Wayland)
sudo dnf install xclip       # Fedora
```

### 📋 Copiar al portapapeles por SSH

Cuando ejecutás la TUI en una máquina remota vía **SSH**, las herramientas del host
remoto (`xclip`/`wl-copy`/arboard) **no** pueden llegar al portapapeles de tu máquina
local. Hay tres caminos, según tu terminal:

#### 1. Opción `7` del menú — Print to terminal (funciona en CUALQUIER terminal) ✅

La forma **garantizada** de copiar logs (incluso muchas líneas) sobre SSH, sin importar
el terminal: abrí el menú de portapapeles (`c`) y elegí **`7` (Print to terminal)**. La
TUI sale temporalmente al scrollback normal del terminal e imprime los logs (respetando
el filtro de nivel activo). Ahí **seleccionás con el mouse y copiás con `Ctrl+Shift+C`**
(podés scrollear para abarcar líneas que no entran en una pantalla). `Enter` vuelve a la
TUI. Asegurate de tener el scrollback del terminal en un tamaño suficiente (idealmente
"ilimitado").

#### 2. Selección nativa con el mouse (lo visible)

La TUI **no captura el mouse**, así que en cualquier momento podés seleccionar el texto
visible de los logs con el mouse y copiar con **`Ctrl+Shift+C`**. Para una selección más
limpia (sin bordes del panel) usá la vista de logs expandida (tecla `F`). Limitación:
solo copia lo que está en pantalla.

#### 3. OSC 52 (terminales compatibles)

Las opciones **1–6** del menú emiten **OSC 52**, una secuencia de escape que el terminal
local intercepta y vuelca a tu portapapeles atravesando SSH. Se usa automáticamente sobre
SSH (forzable con `DOCKER_MANAGER_CLIPBOARD=osc52|local`).

> ⚠️ **GNOME Terminal / VTE NO soporta OSC 52** (Tilix, xfce4-terminal, Ptyxis y Black Box
> también son VTE → tampoco). Es una limitación upstream de larga data:
> https://gitlab.gnome.org/GNOME/vte/-/issues/2495 — si usás GNOME Terminal, usá la
> **opción 7** o la selección nativa.

OSC 52 **sí** funciona en: **kitty, alacritty, wezterm, foot** (Wayland), **ghostty,
iTerm2** y **Konsole** (con "permitir que los programas escriban al portapapeles"). Para
verificar tu terminal:
```bash
printf '\033]52;c;%s\007' "$(printf 'osc52-works' | base64 -w0)"
# Pegá (Ctrl+V): si aparece "osc52-works", tu terminal soporta OSC 52.
```
tmux/screen: la secuencia se envuelve automáticamente en el passthrough (tmux requiere
`allow-passthrough on`, default en ≥ 3.3). Límite práctico ~100 KB; para volúmenes
mayores usá la opción **7** o **Export** (6).

## 🏗️ Arquitectura

```
docker-manager-rust/
├── src/
│   ├── main.rs           # Punto de entrada y CLI
│   ├── docker/
│   │   └── mod.rs        # Cliente Docker API (bollard)
│   ├── ui/
│   │   ├── mod.rs        # Sistema de UI principal
│   │   └── app.rs        # Estado y lógica de la aplicación
│   ├── utils/
│   │   └── clipboard.rs  # Gestión del portapapeles
│   └── config/
│       └── mod.rs        # Configuración persistente
├── Cargo.toml            # Dependencias y metadatos
└── README.md             # Esta documentación
```

## 🚀 Rendimiento

### Comparación con v2.2 (Bash)

| Métrica | Bash v2.2 | Rust v3.0 | Mejora |
|---------|-----------|-----------|---------|
| Tiempo de inicio | ~500ms | ~50ms | 10x |
| Uso de CPU (idle) | 5-10% | <1% | 10x |
| Uso de memoria | ~50MB | ~10MB | 5x |
| Actualización UI | 500ms | 50ms | 10x |
| Manejo de logs | 1000 líneas | Ilimitado | ∞ |

### ⚡ Optimizaciones v3.0.0 (2025-01-22)

**Problemas resueltos**:
- ✅ **Memory leak eliminado**: Tasks de logs/stats con lifecycle management explícito
- ✅ **Batch processing**: Máximo 50 logs por ciclo para evitar lag en UI
- ✅ **Refresh adaptativo**: 250ms (activo) → 500ms (running) → 1000ms (stopped)
- ✅ **Debouncing**: Renders solo cuando hay cambios reales (60 FPS máximo)
- ✅ **Cleanup garantizado**: Abort automático de tasks al cambiar contenedor

**Mejoras de performance**:
- CPU usage reducido en **60%**
- Memoria estable sin crecimiento progresivo (**40%** menos)
- Zero visual glitches después de uso prolongado
- Responsive hasta con logs de alta frecuencia

Ver [OPTIMIZATIONS.md](./OPTIMIZATIONS.md) para detalles técnicos completos.

### 🎨 Correcciones Visuales v3.0.1 (2025-01-22)

**Problema resuelto**: Residuos visuales al cambiar entre contenedores o vistas

**Solución implementada**:
- ✅ **Clear screen garantizado**: Limpieza completa del terminal buffer en cada transición
- ✅ **Loading screens**: Feedback visual durante cambios de estado (100ms)
- ✅ **Transiciones profesionales**: Sistema de estados explícito para cambios
- ✅ **Force redraw**: Override de debouncing en transiciones críticas

**Experiencia de usuario mejorada**:
- Pantallas de loading contextuales ("Switching container...", "Loading logs...")
- Zero residuos visuales al navegar entre contenedores
- Transiciones limpias entre Logs ↔ Stats
- Cambios fluidos al modo expandido

Ver [VISUAL_FIXES.md](./VISUAL_FIXES.md) para detalles técnicos completos.

## 📝 Licencia

MIT - Libre para uso comercial y personal

## 🤝 Contribuciones

Las contribuciones son bienvenidas. Por favor:
1. Fork el proyecto
2. Crea una rama para tu feature
3. Commit tus cambios
4. Push a la rama
5. Abre un Pull Request

## 📬 Soporte

Para reportar bugs o solicitar features, abre un issue en el repositorio.

---

**Docker Manager v3.0** - Gestión de contenedores Docker rápida, eficiente y sin problemas visuales.