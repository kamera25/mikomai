#!/bin/bash
# Remove reproducible Mikomai build products, keeping source and runtime data.
set -eo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
cd "$PROJECT_ROOT"
DRY_RUN=false
DEEP_CLEAN=false
ASSUME_YES=false
show_help() {
    cat <<'HELP'
Usage: ./clean.sh [options]
  -n, --dry-run  List exact paths and sizes without deleting or prompting
  -d, --deep     Also remove node_modules, venv and .fastembed_cache
  -y, --yes      Delete without confirmation
  -h, --help     Show help

Standard cleanup includes Rust, Swift, C# and packaging outputs, Python caches,
and known Mikomai temporary build directories. Models, documents, runtime data,
Git history and logs are kept. Stop builds before running this script.
Custom external build/cache paths are not deleted automatically.
HELP
}
while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--dry-run) DRY_RUN=true ;;
        -d|--deep) DEEP_CLEAN=true ;;
        -y|--yes) ASSUME_YES=true ;;
        -h|--help) show_help; exit 0 ;;
        *) printf 'Unknown option: %s\n' "$1" >&2; exit 1 ;;
    esac
    shift
done
TARGETS=()
add_target() {
    local path="$1" existing
    [[ -e "$path" || -L "$path" ]] || return 0
    for existing in "${TARGETS[@]}"; do
        [[ "$path" == "$existing" ]] && return 0
    done
    TARGETS+=("$path")
}
for path in target .build build dist coverage mikomai-desktop-mac/.build \
    mikomai-desktop-mac/dist contracts/csharp/bin contracts/csharp/obj .eslintcache; do
    add_target "$PROJECT_ROOT/$path"
done
if [[ "$DEEP_CLEAN" == true ]]; then
    for path in node_modules venv .fastembed_cache; do
        add_target "$PROJECT_ROOT/$path"
    done
fi
# Do not traverse dependencies, Git internals, runtime assets or build trees.
while IFS= read -r -d '' path; do
    add_target "$PROJECT_ROOT/${path#./}"
done < <(find . \( -type d \( -name .git -o -name .agents -o -name .codex \
    -o -name node_modules -o -name venv -o -name .fastembed_cache \
    -o -name target -o -name .build -o -name build -o -name dist \
    -o -name coverage -o -name nw-docs -o -name assets \) \) -prune -o \
    \( -type d \( -name __pycache__ -o -name .pytest_cache -o -name .mypy_cache \
    -o -name .ruff_cache \) \) -print0 -prune -o \
    \( -type f \( -name '*.pyc' -o -name '*.pyo' -o -name .DS_Store \) \) -print0)
# Only allowlisted build scratch directories, never arbitrary mikomai-* data.
TEMP_ROOTS=(/private/tmp)
if [[ -n "${TMPDIR:-}" && -d "$TMPDIR" ]]; then
    temp_root="$(cd "$TMPDIR" && pwd -P)"
    [[ "$temp_root" == /private/tmp ]] || TEMP_ROOTS+=("$temp_root")
fi
for temp_root in "${TEMP_ROOTS[@]}"; do
    for name in mikomai-swift-app-build mikomai-clang-cache mikomai-swiftpm-cache \
        mikomai-swift-test-build; do
        path="$temp_root/$name"
        if [[ -O "$path" ]]; then add_target "$path"; fi
    done
    for path in "$temp_root"/mikomai-generated.*; do
        if [[ -d "$path" && -O "$path" ]]; then add_target "$path"; fi
    done
done
if [[ ${#TARGETS[@]} -eq 0 ]]; then
    echo 'No build or temporary artifacts found.'
    exit 0
fi
# Never delete tracked source, even if a future layout adds it under a target.
for path in "${TARGETS[@]}"; do
    case "$path" in
        "$PROJECT_ROOT"/*)
            tracked="$(git -c core.fsmonitor=false ls-files -- "${path#"$PROJECT_ROOT/"}")"
            if [[ -n "$tracked" ]]; then
                printf 'Refusing to delete a path containing tracked files: %s\n' "$path" >&2
                exit 1
            fi ;;
    esac
done
size_kb=0
for path in "${TARGETS[@]}"; do
    kb="$(du -sk "$path" | awk '{print $1}')"
    size_kb=$((size_kb + kb))
    printf '%10s KB  %s\n' "$kb" "$path"
done
awk -v kb="$size_kb" 'BEGIN {printf "Selected artifacts: %.2f GiB\n", kb / 1048576}'
if [[ "$DRY_RUN" == true ]]; then
    echo 'Dry run completed; no files deleted.'
    exit 0
fi
if [[ "$ASSUME_YES" == false ]]; then
    if [[ ! -t 0 ]]; then
        echo 'Use --yes for non-interactive deletion, or --dry-run to inspect.' >&2
        exit 1
    fi
    read -r -p 'Delete the listed artifacts? [y/N]: ' response
    case "$response" in y|Y|yes|YES) ;; *) echo 'Cleanup cancelled.'; exit 0 ;; esac
fi
failures=0
for path in "${TARGETS[@]}"; do
    printf 'Deleting: %s\n' "$path"
    if ! rm -rf -- "$path"; then
        failures=$((failures + 1))
    fi
done
if [[ "$failures" -gt 0 ]]; then
    printf 'Cleanup incomplete: %s deletion(s) failed.\n' "$failures" >&2
    exit 1
fi
awk -v kb="$size_kb" 'BEGIN {printf "Cleanup complete; removed approximately %.2f GiB of artifacts.\n", kb / 1048576}'
