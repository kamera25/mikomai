#!/bin/bash

# ==============================================================================
# Project Cache & Build Artifact Cleaner
# ==============================================================================
# This script scans and deletes unnecessary cache and build files in the project.
# It supports dry-run mode, deep cleaning, and interactive prompts.

# Exit on error for safety, but allow script to continue if some deletions fail
set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Project Root (absolute path of this script's directory)
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_ROOT"

# Configuration defaults
DRY_RUN=false
DEEP_CLEAN=false
ASSUME_YES=false

# Targets definition
STANDARD_TARGETS=(
    "src-tauri/target"
    "dist"
    "build"
    "coverage"
    ".eslintcache"
)

DEEP_TARGETS=(
    "src-tauri/.fastembed_cache"
    "node_modules"
    "venv"
)

RECURSIVE_DIRS=(
    "__pycache__"
    ".pytest_cache"
    ".mypy_cache"
    ".ruff_cache"
)

RECURSIVE_FILES=(
    "*.pyc"
    "*.pyo"
    "*.pyd"
    ".DS_Store"
    "*.log"
    "npm-debug.log*"
    "yarn-debug.log*"
    "yarn-error.log*"
    "pnpm-debug.log*"
)

# Display help menu
show_help() {
    echo -e "${BOLD}Project Cache & Build Artifact Cleaner${NC}"
    echo "Usage: ./clean.sh [options]"
    echo ""
    echo "Options:"
    echo "  -d, --deep      Include deep clean targets (node_modules, venv, .fastembed_cache)"
    echo "  -n, --dry-run   Perform a dry run (show files and sizes, do not delete)"
    echo "  -y, --yes       Assume yes to all prompts (non-interactive mode)"
    echo "  -h, --help      Show this help message"
}

# Parse command line arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        -d|--deep) DEEP_CLEAN=true ;;
        -n|--dry-run) DRY_RUN=true ;;
        -y|--yes) ASSUME_YES=true ;;
        -h|--help) show_help; exit 0 ;;
        *) echo -e "${RED}Error: Unknown parameter '$1'${NC}"; show_help; exit 1 ;;
    esac
    shift
done

# Function to get size of a path in KB
get_size_kb() {
    local path="$1"
    if [ -e "$path" ]; then
        du -sk "$path" 2>/dev/null | cut -f1
    else
        echo 0
    fi
}

# Function to format size in KB to human-readable format
format_size() {
    local kb=$1
    awk -v kb="$kb" '
    BEGIN {
        if (kb >= 1048576) {
            printf "%.2f GB\n", kb / 1048576
        } else if (kb >= 1024) {
            printf "%.2f MB\n", kb / 1024
        } else {
            printf "%d KB\n", kb
        }
    }'
}

# Function to calculate size of recursively searched files/dirs (pruning heavy directories for speed)
get_recursive_size_kb() {
    local total=0
    
    # Calculate directory sizes
    for dir in "${RECURSIVE_DIRS[@]}"; do
        local sz
        sz=$(find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type d -name "$dir" -exec du -sk {} + 2>/dev/null | awk '{s+=$1} END {print s}')
        if [ -n "$sz" ]; then
            total=$((total + sz))
        fi
    done
    
    # Calculate file sizes
    for pat in "${RECURSIVE_FILES[@]}"; do
        local sz
        sz=$(find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type f -name "$pat" -exec du -sk {} + 2>/dev/null | awk '{s+=$1} END {print s}')
        if [ -n "$sz" ]; then
            total=$((total + sz))
        fi
    done
    
    echo "$total"
}

# Scan sizes
echo -e "${BLUE}Scanning project directories for cache and build artifacts...${NC}"

size_standard_kb=0
for target in "${STANDARD_TARGETS[@]}"; do
    size_standard_kb=$((size_standard_kb + $(get_size_kb "$target")))
done

size_recursive_kb=$(get_recursive_size_kb)
size_deep_kb=0
for target in "${DEEP_TARGETS[@]}"; do
    size_deep_kb=$((size_deep_kb + $(get_size_kb "$target")))
done

# Set interactive mode
if [ -t 0 ] && [ "$ASSUME_YES" = false ]; then
    INTERACTIVE=true
else
    INTERACTIVE=false
fi

# If interactive and not already set to deep, ask the user
if [ "$INTERACTIVE" = true ] && [ "$DEEP_CLEAN" = false ]; then
    # Display potential savings with and without deep clean
    echo -e "\n${BOLD}Scan Summary:${NC}"
    echo -e "  - Standard Cache & Build Artifacts: $(format_size $((size_standard_kb + size_recursive_kb)))"
    echo -e "  - Deep Clean Targets (node_modules, venv, .fastembed_cache): $(format_size $size_deep_kb)"
    echo ""
    echo -e -n "${YELLOW}Would you like to include deep clean targets (dependencies & download caches)? [y/N]: ${NC}"
    read -r response
    if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        DEEP_CLEAN=true
        echo -e "${GREEN}Deep clean enabled.${NC}"
    else
        echo -e "${BLUE}Standard clean selected (dependencies and model caches will be kept).${NC}"
    fi
fi

# Calculate total space to free
if [ "$DEEP_CLEAN" = true ]; then
    total_to_free_kb=$((size_standard_kb + size_recursive_kb + size_deep_kb))
else
    total_to_free_kb=$((size_standard_kb + size_recursive_kb))
fi

# Display scan details
echo -e "\n${BOLD}Target Scan Results:${NC}"
echo -e "--------------------------------------------------"
echo -e "${BLUE}${BOLD}[Standard Cleanup Targets]${NC}"
for target in "${STANDARD_TARGETS[@]}"; do
    if [ -e "$target" ]; then
        printf "  %-30s %s\n" "$target" "$(format_size $(get_size_kb "$target"))"
    fi
done
if [ "$size_recursive_kb" -gt 0 ]; then
    printf "  %-30s %s\n" "Temporary & PyCache files" "$(format_size $size_recursive_kb)"
fi

echo -e "\n${YELLOW}${BOLD}[Deep Cleanup Targets]${NC} $([ "$DEEP_CLEAN" = true ] && echo -e "${GREEN}(Selected)${NC}" || echo "(Not Selected)")"
for target in "${DEEP_TARGETS[@]}"; do
    if [ -e "$target" ]; then
        printf "  %-30s %s\n" "$target" "$(format_size $(get_size_kb "$target"))"
    fi
done
echo -e "--------------------------------------------------"
echo -e "${BOLD}Total Space to Free: $(format_size $total_to_free_kb)${NC}"

# If total to free is 0, exit early
if [ "$total_to_free_kb" -eq 0 ]; then
    echo -e "\n${GREEN}Project is already clean! No cache or build files found.${NC}"
    exit 0
fi

# Confirm deletion
if [ "$INTERACTIVE" = true ]; then
    echo -e -n "\n${RED}${BOLD}Are you sure you want to proceed with the deletion? [y/N]: ${NC}"
    read -r response
    if [[ ! "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        echo -e "${BLUE}Cleanup cancelled. No files were deleted.${NC}"
        exit 0
    fi
fi

# Perform cleanup
echo -e "\n${BLUE}Starting cleanup...${NC}"

# Prevent exit on deletion error so we can clean as much as possible
set +e

# Clean standard targets
for target in "${STANDARD_TARGETS[@]}"; do
    if [ -e "$target" ]; then
        if [ "$DRY_RUN" = true ]; then
            echo -e "  [Dry Run] Would delete: $target"
        else
            echo -e "  Deleting: $target..."
            rm -rf "$target"
        fi
    fi
done

# Clean recursive targets
if [ "$DRY_RUN" = true ]; then
    for dir in "${RECURSIVE_DIRS[@]}"; do
        count=$(find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type d -name "$dir" -print 2>/dev/null | wc -l | tr -d ' ')
        if [ "$count" -gt 0 ]; then
            echo -e "  [Dry Run] Would delete: $count directory/ies named '$dir'"
        fi
    done
    for pat in "${RECURSIVE_FILES[@]}"; do
        count=$(find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type f -name "$pat" -print 2>/dev/null | wc -l | tr -d ' ')
        if [ "$count" -gt 0 ]; then
            echo -e "  [Dry Run] Would delete: $count file(s) matching '$pat'"
        fi
    done
else
    echo -e "  Cleaning temporary, log, and cache files recursively..."
    for dir in "${RECURSIVE_DIRS[@]}"; do
        find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type d -name "$dir" -exec rm -rf {} + 2>/dev/null
    done
    for pat in "${RECURSIVE_FILES[@]}"; do
        find . \( -name "node_modules" -o -name "venv" -o -name "target" -o -name ".fastembed_cache" \) -prune -o -type f -name "$pat" -exec rm -f {} + 2>/dev/null
    done
fi

# Clean deep targets if requested
if [ "$DEEP_CLEAN" = true ]; then
    for target in "${DEEP_TARGETS[@]}"; do
        if [ -e "$target" ]; then
            if [ "$DRY_RUN" = true ]; then
                echo -e "  [Dry Run] Would delete (Deep Clean): $target"
            else
                echo -e "  Deleting (Deep Clean): $target..."
                rm -rf "$target"
            fi
        fi
    done
fi

if [ "$DRY_RUN" = true ]; then
    echo -e "\n${GREEN}[Dry Run Completed] No files were actually deleted.${NC}"
else
    echo -e "\n${GREEN}Cleanup completed successfully! Freed approximately $(format_size $total_to_free_kb) of disk space.${NC}"
fi
