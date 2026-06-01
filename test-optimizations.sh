#!/bin/bash

# Test script para verificar las optimizaciones de Dockpit v3.0

set -e

echo "🔍 Dockpit v3.0 - Verificación de Optimizaciones"
echo "========================================================"
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if dockpit is built
if [ ! -f "./target/release/dockpit" ]; then
    echo -e "${YELLOW}⚠️  Ejecutable no encontrado. Compilando...${NC}"
    cargo build --release
    echo ""
fi

echo "✅ Verificaciones básicas:"
echo ""

# 1. Check binary size
BINARY_SIZE=$(du -h ./target/release/dockpit | cut -f1)
echo "📦 Tamaño del ejecutable: $BINARY_SIZE"

# 2. Check dependencies
echo "🔗 Verificando dependencias..."
if ldd ./target/release/dockpit | grep -q "not found"; then
    echo -e "${RED}❌ Faltan dependencias${NC}"
    ldd ./target/release/dockpit
    exit 1
else
    echo -e "${GREEN}✅ Todas las dependencias presentes${NC}"
fi

# 3. Check Docker connection
echo ""
echo "🐳 Verificando conexión a Docker..."
if ! docker ps > /dev/null 2>&1; then
    echo -e "${RED}❌ Docker no está corriendo o no tienes permisos${NC}"
    echo "   Ejecuta: sudo systemctl start docker"
    echo "   O añade tu usuario al grupo docker: sudo usermod -aG docker \$USER"
    exit 1
else
    CONTAINER_COUNT=$(docker ps -a | wc -l)
    echo -e "${GREEN}✅ Docker OK - $(($CONTAINER_COUNT - 1)) contenedores disponibles${NC}"
fi

# 4. Quick performance test
echo ""
echo "⚡ Test rápido de performance:"
echo ""

# Startup time
echo "⏱️  Midiendo tiempo de inicio..."
START_TIME=$(date +%s.%N)
timeout 1 ./target/release/dockpit list > /dev/null 2>&1 || true
END_TIME=$(date +%s.%N)
STARTUP_TIME=$(echo "$END_TIME - $START_TIME" | bc)
echo "   Tiempo de inicio: ${STARTUP_TIME}s"

if (( $(echo "$STARTUP_TIME < 0.5" | bc -l) )); then
    echo -e "   ${GREEN}✅ Excelente (<500ms)${NC}"
else
    echo -e "   ${YELLOW}⚠️  Aceptable pero puede mejorar${NC}"
fi

# 5. Memory baseline
echo ""
echo "💾 Uso de memoria (baseline):"
MEMORY_BASELINE=$(ps aux | grep dockpit | grep -v grep | awk '{print $6}' | head -1)
if [ -n "$MEMORY_BASELINE" ]; then
    MEMORY_MB=$(echo "scale=2; $MEMORY_BASELINE / 1024" | bc)
    echo "   Memoria actual: ${MEMORY_MB} MB"
else
    echo "   No hay proceso dockpit corriendo actualmente"
fi

echo ""
echo "📋 Tests manuales recomendados:"
echo "================================"
echo ""
echo "1. Test de estabilidad de memoria:"
echo "   - Ejecutar: ./target/release/dockpit"
echo "   - Navegar entre contenedores 20+ veces"
echo "   - Dejar corriendo por 30+ minutos"
echo "   - Verificar que memoria no crece indefinidamente"
echo ""
echo "2. Test de CPU con contenedor activo:"
echo "   - Seleccionar contenedor con logs frecuentes"
echo "   - Observar CPU usage (debería ser <10%)"
echo "   - Cambiar a modo Stats y verificar"
echo ""
echo "3. Test de navegación rápida:"
echo "   - Usar teclas numéricas 1-9 para saltar entre contenedores"
echo "   - No debería haber lag ni visual glitches"
echo ""
echo "4. Test de logs de alta frecuencia:"
echo "   - Ver logs de contenedor con output constante"
echo "   - Scroll debería ser fluido"
echo "   - Sin stuttering ni freezes"
echo ""
echo "💡 Comandos útiles para monitoring:"
echo "===================================="
echo ""
echo "# Ver uso de CPU/memoria del proceso:"
echo "watch -n 1 'ps aux | grep dockpit | grep -v grep'"
echo ""
echo "# Monitoring más detallado:"
echo "htop -p \$(pgrep dockpit)"
echo ""
echo "# Test de memory leaks (requiere valgrind):"
echo "valgrind --leak-check=full --show-leak-kinds=all ./target/release/dockpit list"
echo ""
echo "✅ Verificación completa"
echo ""
echo "🚀 Para ejecutar el gestor interactivo:"
echo "   ./target/release/dockpit"
echo ""
