#!/bin/bash

# Check for unused imports and unused files in the codebase
# Usage: ./scripts/check-unused.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "🔍 Checking for unused code in Deduparr codebase..."
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

HAS_ISSUES=0

# ============================================================================
# FRONTEND CHECKS
# ============================================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Frontend Checks (TypeScript/React)${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

cd "$ROOT_DIR/frontend"

# Check for unused imports using ESLint
echo ""
echo -e "${YELLOW}📦 Checking for unused imports in frontend...${NC}"
if npm run lint 2>&1 | grep -i "unused\|never used\|defined but never" > /tmp/unused-imports.txt; then
    echo -e "${RED}❌ Found unused imports:${NC}"
    cat /tmp/unused-imports.txt
    HAS_ISSUES=1
else
    echo -e "${GREEN}✅ No unused imports found${NC}"
fi

# Check for unused TypeScript files
echo ""
echo -e "${YELLOW}📄 Checking for unused TypeScript files...${NC}"
UNUSED_FILES=()

# Install ts-prune if not available (lightweight unused exports checker)
if ! command -v ts-prune &> /dev/null; then
    echo "Installing ts-prune temporarily..."
    npm install -D ts-prune --no-save > /dev/null 2>&1
fi

# Run ts-prune to find unused exports
if npx ts-prune --error 2>&1 | grep -v "used in module" > /tmp/ts-prune-output.txt; then
    if [ -s /tmp/ts-prune-output.txt ]; then
        echo -e "${YELLOW}⚠️  Found potentially unused exports:${NC}"
        cat /tmp/ts-prune-output.txt
        echo ""
        echo -e "${YELLOW}Note: Some exports may be used dynamically or in tests${NC}"
    else
        echo -e "${GREEN}✅ No unused exports found${NC}"
    fi
else
    echo -e "${GREEN}✅ No unused exports found${NC}"
fi

# Check for orphaned component files (not imported anywhere)
echo ""
echo -e "${YELLOW}🔗 Checking for orphaned files...${NC}"
ORPHANED=0

# Find all TypeScript/TSX files except main.tsx, vite-env.d.ts, App.tsx and test files
for file in $(find src -type f \( -name "*.ts" -o -name "*.tsx" \) \
    ! -name "main.tsx" \
    ! -name "App.tsx" \
    ! -name "vite-env.d.ts" \
    ! -name "*.test.ts" \
    ! -name "*.test.tsx" \
    ! -name "*.spec.ts" \
    ! -name "*.spec.tsx"); do
    
    # Get the base filename without extension
    basename=$(basename "$file" .tsx)
    basename=$(basename "$basename" .ts)
    
    # Skip if it's an index file (entry points)
    if [ "$basename" = "index" ]; then
        continue
    fi
    
    # Check if this file is imported anywhere (including @/ alias)
    if ! grep -rq "from.*['\"].*/$basename['\"]" src/ && \
       ! grep -rq "from.*['\"]@/.*/$basename['\"]" src/ && \
       ! grep -rq "from.*['\"].*$(basename $(dirname $file))/$basename['\"]" src/ && \
       ! grep -rq "import.*$basename" src/; then
        echo -e "${YELLOW}  ⚠️  Possibly orphaned: $file${NC}"
        ORPHANED=$((ORPHANED + 1))
    fi
done

if [ $ORPHANED -eq 0 ]; then
    echo -e "${GREEN}✅ No obvious orphaned files found${NC}"
else
    echo -e "${YELLOW}Found $ORPHANED potentially orphaned file(s)${NC}"
    echo -e "${YELLOW}Note: Review manually - may be used via path aliases or dynamic imports${NC}"
fi

# ============================================================================
# BACKEND CHECKS
# ============================================================================
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Backend Checks (Python)${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

cd "$ROOT_DIR/backend"

# Check for unused Python imports using autoflake
echo ""
echo -e "${YELLOW}📦 Checking for unused imports in backend...${NC}"
if command -v autoflake &> /dev/null; then
    UNUSED_IMPORTS=$(autoflake --check --remove-all-unused-imports --recursive app/ 2>&1 | grep "app/" || true)
    if [ -n "$UNUSED_IMPORTS" ]; then
        echo -e "${RED}❌ Found unused imports:${NC}"
        echo "$UNUSED_IMPORTS"
        HAS_ISSUES=1
    else
        echo -e "${GREEN}✅ No unused imports found${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  autoflake not installed, skipping unused import check${NC}"
    echo "   Install with: pip install autoflake"
fi

# Check for unused Python variables using pylint or ruff
echo ""
echo -e "${YELLOW}📊 Checking for unused variables in backend...${NC}"
if command -v ruff &> /dev/null; then
    if ruff check app/ --select F401,F841 --quiet 2>&1 > /tmp/ruff-unused.txt; then
        echo -e "${GREEN}✅ No unused variables found${NC}"
    else
        if [ -s /tmp/ruff-unused.txt ]; then
            echo -e "${RED}❌ Found unused variables/imports:${NC}"
            cat /tmp/ruff-unused.txt
            HAS_ISSUES=1
        else
            echo -e "${GREEN}✅ No unused variables found${NC}"
        fi
    fi
else
    echo -e "${YELLOW}⚠️  ruff not installed, skipping unused variable check${NC}"
fi

# Check for orphaned Python files
echo ""
echo -e "${YELLOW}🔗 Checking for orphaned Python files...${NC}"
ORPHANED_PY=0

# Find all Python files except __init__.py, main.py, and test files
for file in $(find app -type f -name "*.py" \
    ! -name "__init__.py" \
    ! -name "main.py" \
    ! -name "test_*.py" \
    ! -name "conftest.py"); do
    
    # Get the module name and relative path
    basename=$(basename "$file" .py)
    relative_path=$(echo "$file" | sed 's|^app/||' | sed 's|\.py$||' | sed 's|/|.|g')
    
    # Check if imported via full module path, basename, or __init__.py
    if ! grep -rq "from.*$relative_path" app/ tests/ && \
       ! grep -rq "import.*$relative_path" app/ tests/ && \
       ! grep -rq "from.*import.*$basename" app/ tests/ && \
       ! grep -rq "import.*$basename" app/ tests/ && \
       ! grep -rq "$basename" app/services/__init__.py app/api/__init__.py app/models/__init__.py 2>/dev/null; then
        # Double-check: might be imported in __init__.py
        dir=$(dirname "$file")
        if [ -f "$dir/__init__.py" ]; then
            if ! grep -q "$basename" "$dir/__init__.py"; then
                echo -e "${YELLOW}  ⚠️  Possibly orphaned: $file${NC}"
                ORPHANED_PY=$((ORPHANED_PY + 1))
            fi
        else
            echo -e "${YELLOW}  ⚠️  Possibly orphaned: $file${NC}"
            ORPHANED_PY=$((ORPHANED_PY + 1))
        fi
    fi
done

if [ $ORPHANED_PY -eq 0 ]; then
    echo -e "${GREEN}✅ No obvious orphaned Python files found${NC}"
else
    echo -e "${YELLOW}Found $ORPHANED_PY potentially orphaned file(s)${NC}"
    echo -e "${YELLOW}Note: Review manually - may be imported via __init__.py or used dynamically${NC}"
fi

# ============================================================================
# SUMMARY
# ============================================================================
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

if [ $HAS_ISSUES -eq 0 ]; then
    echo -e "${GREEN}✨ No critical unused code issues found!${NC}"
    echo ""
    echo "To automatically remove unused imports:"
    echo "  Frontend: npm run lint:fix (in frontend/)"
    echo "  Backend:  autoflake --remove-all-unused-imports --in-place --recursive app/"
else
    echo -e "${RED}⚠️  Found unused code issues. Please review and clean up.${NC}"
    exit 1
fi

echo ""
