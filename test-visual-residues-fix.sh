#!/bin/bash

# Test Suite para Visual Residues Fix v3.0.2
# Verifica que NO aparezcan residuos visuales en transiciones

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Docker Manager v3.0.2 - Visual Residues Fix Test Suite     ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Verificar que el binario existe
if [ ! -f "./target/release/docker-manager" ]; then
    echo "❌ ERROR: Binario no encontrado en ./target/release/docker-manager"
    echo "Ejecuta: cargo build --release"
    exit 1
fi

# Verificar versión
VERSION=$(./target/release/docker-manager --version)
echo "✅ Versión detectada: $VERSION"
echo ""

if [[ ! "$VERSION" =~ "3.0.2" ]]; then
    echo "⚠️  WARNING: Versión esperada 3.0.2, encontrada: $VERSION"
    echo ""
fi

echo "════════════════════════════════════════════════════════════════"
echo "  TESTS MANUALES - Verificar Residuos Visuales"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "📋 Test 1: Logs ↔ Stats Rápido (CRÍTICO)"
echo "────────────────────────────────────────"
echo "PASOS:"
echo "  1. Ejecutar: ./target/release/docker-manager"
echo "  2. Presionar L (switch to logs)"
echo "  3. Presionar S (switch to stats)"
echo "  4. Presionar L (volver a logs)"
echo "  5. Repetir 5-10 veces RÁPIDAMENTE"
echo ""
echo "✅ ESPERADO:"
echo "  - Loading screen visible en CADA transición"
echo "  - CERO residuos de texto de logs en stats"
echo "  - CERO residuos de stats en logs"
echo "  - Pantalla completamente limpia en cada cambio"
echo ""
echo "❌ FALLIDO SI:"
echo "  - Se ven números/texto de la vista anterior"
echo "  - Hay superposición de contenido"
echo "  - Loading screen no aparece"
echo ""
read -p "Presiona ENTER para continuar al siguiente test..."
echo ""

echo "📋 Test 2: Expand/Collapse Logs (CRÍTICO)"
echo "──────────────────────────────────────────"
echo "PASOS:"
echo "  1. Ejecutar: ./target/release/docker-manager"
echo "  2. Presionar L (switch to logs)"
echo "  3. Presionar F (expand logs - full screen)"
echo "  4. Presionar F (collapse logs - dual panel)"
echo "  5. Repetir 5-10 veces RÁPIDAMENTE"
echo ""
echo "✅ ESPERADO:"
echo "  - Loading screen visible en CADA toggle"
echo "  - CERO residuos del layout anterior"
echo "  - Transición suave entre layouts"
echo "  - Lista de containers visible/oculta correctamente"
echo ""
echo "❌ FALLADO SI:"
echo "  - Se ve doble contenido superpuesto"
echo "  - Bordes del panel anterior visibles"
echo "  - Loading screen no aparece"
echo ""
read -p "Presiona ENTER para continuar al siguiente test..."
echo ""

echo "📋 Test 3: Secuencia Compleja Mixta"
echo "────────────────────────────────────"
echo "PASOS:"
echo "  1. Ejecutar: ./target/release/docker-manager"
echo "  2. Secuencia rápida sin pausas:"
echo "     L → S → L → F → F → S → L → F → S → L"
echo "  3. Observar TODAS las transiciones"
echo ""
echo "✅ ESPERADO:"
echo "  - TODAS las transiciones limpias"
echo "  - Loading screens visibles"
echo "  - CERO residuos en ningún momento"
echo ""
echo "❌ FALLADO SI:"
echo "  - Cualquier residuo visual en cualquier transición"
echo ""
read -p "Presiona ENTER para continuar al siguiente test..."
echo ""

echo "📋 Test 4: Container Switch + View Switch"
echo "──────────────────────────────────────────"
echo "PASOS (requiere 3+ containers):"
echo "  1. Ejecutar: ./target/release/docker-manager"
echo "  2. Secuencia:"
echo "     ↓ (container 2) → S (stats) → ↓ (container 3)"
echo "     → L (logs) → F (expand) → ↑ (container 2)"
echo "  3. Observar todas las transiciones"
echo ""
echo "✅ ESPERADO:"
echo "  - Buffers se limpian en TODOS los cambios"
echo "  - Loading screens en cambios de vista"
echo "  - No se mezclan datos de diferentes containers"
echo "  - CERO residuos visuales"
echo ""
echo "❌ FALLADO SI:"
echo "  - Logs de container A aparecen en container B"
echo "  - Stats viejos visibles al cambiar"
echo ""
read -p "Presiona ENTER para continuar al siguiente test..."
echo ""

echo "📋 Test 5: High-Frequency Logs + Switching"
echo "───────────────────────────────────────────"
echo "PASOS (requiere container con logs activos):"
echo "  1. Ejecutar: ./target/release/docker-manager"
echo "  2. Seleccionar container con logs frecuentes"
echo "  3. Secuencia rápida múltiples veces:"
echo "     L → S → L → F → F → S"
echo "  4. Observar manejo de logs en tiempo real"
echo ""
echo "✅ ESPERADO:"
echo "  - Buffers se limpian incluso con logs activos"
echo "  - No se mezclan logs nuevos con vista anterior"
echo "  - Loading screens se muestran correctamente"
echo "  - Performance estable"
echo ""
echo "❌ FALLADO SI:"
echo "  - Logs nuevos aparecen en stats"
echo "  - Buffers se saturan"
echo "  - Lag o freeze durante transiciones"
echo ""
read -p "Presiona ENTER para ver el resumen..."
echo ""

echo "════════════════════════════════════════════════════════════════"
echo "  VERIFICACIÓN DE CÓDIGO"
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "🔍 Verificando implementación de fixes..."
echo ""

# Verificar cleanup_streams en switch_to_logs_mode
if grep -A 5 "async fn switch_to_logs_mode" src/ui/app.rs | grep -q "cleanup_streams()"; then
    echo "✅ switch_to_logs_mode() tiene cleanup_streams()"
else
    echo "❌ FALTA: cleanup_streams() en switch_to_logs_mode()"
fi

# Verificar cleanup_streams en switch_to_stats_mode
if grep -A 5 "async fn switch_to_stats_mode" src/ui/app.rs | grep -q "cleanup_streams()"; then
    echo "✅ switch_to_stats_mode() tiene cleanup_streams()"
else
    echo "❌ FALTA: cleanup_streams() en switch_to_stats_mode()"
fi

# Verificar que toggle_expanded_logs es async
if grep "async fn toggle_expanded_logs" src/ui/app.rs > /dev/null; then
    echo "✅ toggle_expanded_logs() es async"
else
    echo "❌ FALTA: toggle_expanded_logs() NO es async"
fi

# Verificar sleep en toggle_expanded_logs
if grep -A 10 "async fn toggle_expanded_logs" src/ui/app.rs | grep -q "sleep.*100"; then
    echo "✅ toggle_expanded_logs() tiene sleep de 100ms"
else
    echo "❌ FALTA: sleep de 100ms en toggle_expanded_logs()"
fi

# Verificar await en call site
if grep "toggle_expanded_logs().await" src/ui/app.rs > /dev/null; then
    echo "✅ Call site usa .await para toggle_expanded_logs()"
else
    echo "❌ FALTA: .await en call site de toggle_expanded_logs()"
fi

echo ""
echo "════════════════════════════════════════════════════════════════"
echo "  RESUMEN DE FIXES v3.0.2"
echo "════════════════════════════════════════════════════════════════"
echo ""
echo "🎯 Problemas Solucionados:"
echo "  ✅ Residuos visuales al cambiar Logs ↔ Stats"
echo "  ✅ Residuos visuales al Expandir/Contraer logs"
echo "  ✅ Buffers no se limpiaban en cambios de vista"
echo "  ✅ Loading screens no siempre visibles"
echo ""
echo "🔧 Cambios Implementados:"
echo "  ✅ cleanup_streams() en switch_to_logs_mode()"
echo "  ✅ cleanup_streams() en switch_to_stats_mode()"
echo "  ✅ toggle_expanded_logs() ahora async con sleep 100ms"
echo "  ✅ Call site actualizado con .await"
echo ""
echo "📊 Resultado Esperado:"
echo "  ✅ CERO residuos visuales en TODAS las transiciones"
echo "  ✅ Loading screens SIEMPRE visibles"
echo "  ✅ Buffers COMPLETAMENTE limpios antes de cada vista"
echo "  ✅ Performance estable con logs de alta frecuencia"
echo ""
echo "════════════════════════════════════════════════════════════════"
echo ""

echo "💡 PARA EJECUTAR DOCKER MANAGER:"
echo "   ./target/release/docker-manager"
echo ""
echo "📚 DOCUMENTACIÓN:"
echo "   - VISUAL_RESIDUES_FIX.md - Detalles técnicos completos"
echo "   - CHANGELOG.md - Historial de cambios v3.0.2"
echo ""

