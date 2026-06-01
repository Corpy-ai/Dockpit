#!/bin/bash

# Test de Correcciones Visuales para Docker Manager v3.0.1

set -e

echo "🎨 Docker Manager v3.0.1 - Test de Correcciones Visuales"
echo "========================================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if docker-manager is built
if [ ! -f "./target/release/docker-manager" ]; then
    echo -e "${YELLOW}⚠️  Ejecutable no encontrado. Compilando...${NC}"
    cargo build --release
    echo ""
fi

echo "✅ Verificaciones previas:"
echo ""

# Check compilation timestamp
BUILD_TIME=$(stat -c %y ./target/release/docker-manager | cut -d' ' -f1,2)
echo "📦 Última compilación: $BUILD_TIME"

# Check Docker
if ! docker ps > /dev/null 2>&1; then
    echo -e "${RED}❌ Docker no está corriendo${NC}"
    exit 1
else
    CONTAINER_COUNT=$(docker ps -a | wc -l)
    echo -e "${GREEN}✅ Docker OK - $(($CONTAINER_COUNT - 1)) contenedores disponibles${NC}"
fi

echo ""
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${CYAN}📋 Tests Manuales para Verificar Correcciones Visuales${NC}"
echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

echo "🔍 Test 1: Navegación entre Contenedores"
echo "   Objetivo: Verificar que NO hay residuos visuales"
echo "   Pasos:"
echo "     1. Ejecutar: ./target/release/docker-manager"
echo "     2. Presionar ↓ varias veces"
echo "     3. Presionar ↑ varias veces"
echo "   ✅ Esperado:"
echo "     - Ver pantalla 'Switching container...' brevemente"
echo "     - Pantalla completamente limpia después de cada cambio"
echo "     - NO quedan caracteres o bordes del contenedor anterior"
echo ""

echo "🔄 Test 2: Cambio Logs ↔ Stats"
echo "   Objetivo: Verificar transiciones limpias entre vistas"
echo "   Pasos:"
echo "     1. Seleccionar un contenedor running"
echo "     2. Presionar 'L' (Logs)"
echo "     3. Presionar 'S' (Stats)"
echo "     4. Alternar 5+ veces"
echo "   ✅ Esperado:"
echo "     - Ver 'Loading logs...' o 'Loading stats...'"
echo "     - Pantalla limpia en cada cambio"
echo "     - NO hay superposición de contenido"
echo "     - Info de red se muestra correctamente en Stats"
echo ""

echo "📱 Test 3: Modo Expandido"
echo "   Objetivo: Verificar cambios de layout limpios"
echo "   Pasos:"
echo "     1. Estar en vista Logs"
echo "     2. Presionar 'F' (Expandir)"
echo "     3. Presionar 'F' (Contraer)"
echo "     4. Repetir 3+ veces"
echo "   ✅ Esperado:"
echo "     - Ver 'Switching view...'"
echo "     - Layout cambia completamente"
echo "     - NO quedan bordes o paneles del modo anterior"
echo ""

echo "🎯 Test 4: Salto Numérico Directo"
echo "   Objetivo: Verificar cambios rápidos limpios"
echo "   Pasos:"
echo "     1. Presionar '1' (saltar a contenedor #1)"
echo "     2. Presionar '5' (saltar a contenedor #5)"
echo "     3. Presionar '3' (saltar a contenedor #3)"
echo "   ✅ Esperado:"
echo "     - Ver 'Jumping to container #X...'"
echo "     - Cambio instantáneo pero limpio"
echo "     - NO hay residuos visuales en ningún panel"
echo ""

echo "⚡ Test 5: Navegación Rápida (Stress Test)"
echo "   Objetivo: Verificar que loading screens no causan lag"
echo "   Pasos:"
echo "     1. Presionar ↓ muy rápido 10+ veces"
echo "     2. Cambiar L/S rápidamente varias veces"
echo "   ✅ Esperado:"
echo "     - Sistema responsive (no lag perceptible)"
echo "     - Loading screens visibles pero breves (100ms)"
echo "     - Siempre termina con pantalla limpia"
echo ""

echo "🐛 Test 6: Caso Edge - Contenedor Stopped"
echo "   Objetivo: Verificar comportamiento con contenedores no-running"
echo "   Pasos:"
echo "     1. Seleccionar contenedor con estado 'stopped'"
echo "     2. Presionar 'L' (intentar ver logs)"
echo "     3. Presionar 'S' (intentar ver stats)"
echo "   ✅ Esperado:"
echo "     - Mensaje claro: 'Container is not running'"
echo "     - NO hay residuos de intentos de carga fallidos"
echo "     - Pantalla limpia y mensaje centrado"
echo ""

echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

echo "💡 Qué Buscar en los Loading Screens:"
echo "   ✅ Box centrado con borde cyan"
echo "   ✅ Emoji 🔄 visible"
echo "   ✅ Mensaje contextual claro"
echo "   ✅ 'Please wait...' en gris"
echo "   ✅ Duración ~100ms (visible pero no molesto)"
echo ""

echo "❌ Señales de Problemas (reportar si ocurren):"
echo "   🔴 Caracteres residuales después de transiciones"
echo "   🔴 Bordes o paneles superpuestos"
echo "   🔴 Loading screens que duran >500ms"
echo "   🔴 Pantalla corrupta después de cambios rápidos"
echo "   🔴 Lag perceptible en navegación"
echo ""

echo "📊 Comparación Visual Esperada:"
echo ""
echo "   ANTES (v3.0.0):"
echo "   ┌─ Container 1 ─┐"
echo "   │ [log data...] │  ← Usuario cambia"
echo "   │ █og d@ta#...] │  ← RESIDUOS ❌"
echo ""
echo "   AHORA (v3.0.1):"
echo "   ┌─ Container 1 ─┐"
echo "   │ [log data...] │  ← Usuario cambia"
echo "   ┌─ Loading ─┐"
echo "   │ 🔄 Switch..│     ← Feedback 100ms"
echo "   └───────────┘"
echo "   ┌─ Container 2 ─┐"
echo "   │ [log data...] │  ← Pantalla LIMPIA ✅"
echo ""

echo "🚀 Ejecutar Test Interactivo:"
echo "   ./target/release/docker-manager"
echo ""

echo "📝 Después del Testing:"
echo "   Si encuentras residuos visuales:"
echo "   1. Anotar: ¿Qué acción causó el problema?"
echo "   2. Anotar: ¿Qué contenedor estabas viendo?"
echo "   3. Tomar screenshot si es posible"
echo "   4. Reportar en el issue tracker"
echo ""

echo -e "${GREEN}✅ Test preparado - Ejecutar y verificar manualmente${NC}"
echo ""
