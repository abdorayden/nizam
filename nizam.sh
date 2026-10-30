#!/usr/bin/env bash

### nizam manager
###     like maven for java but it's hand written script
###     also with more checks to make sure running and testing are working
###     it uses bash shell script
###     if you are using other shells just run bash nizam

set -euo pipefail

# use run by default
if [[ $# -eq 0 ]]; then
    set -- run
fi

if [[ "${1:-}" == -* ]]; then
    set -- run "$@"
fi

# normalize old flag style (i used to "--no-venv" flag)
for arg in "$@"; do
    if [[ "$arg" == "-no-venv" || "$arg" == "--no-venv" || "$arg" == "-no-venv-init" ]]; then
        set -- "${@/-no-venv/-n}"
        set -- "${@/--no-venv/-n}"
        set -- "${@/-no-venv-init/-n}"
        break
    fi
done

option="${1:-run}"
shift || true

nizam_help() {
    cat <<'EOF'
Usage: ./run.sh {test|run} [options]

  test        Run engine tests with cargo
              -v        Exit on first error (set -e)

  run         Build and run the Python app (via maturin)
              -n        Skip venv creation/initialization (use existing .venv)
              -v        Exit on first error (set -e)

Notes:
  - If no subcommand is given, defaults to "run".
  - Old flags like -no-venv are accepted for backwards compatibility.
EOF
}

nizam_check() {
    local verbose=false
    while getopts ":v" opt; do
        case "$opt" in
            v)
                verbose=true
                ;;
            :)
                echo "Error: -$OPTARG requires an argument" >&2
                exit 1
                ;;
            \?)
                echo "Error: invalid option -$OPTARG" >&2
                nizam_help >&2
                exit 1
                ;;
        esac
    done

    if [[ "$verbose" == true ]]; then
        set -x
    fi

    command -v cargo >/dev/null 2>&1 || { echo "Error: cargo not found (install rustup)" >&2; exit 1; }
    command -v rustc >/dev/null 2>&1 || { echo "Error: rustc not found (install rustup)" >&2; exit 1; }

    cargo test
}

nizam_run() {
    local enable_venv=true
    local verbose=false

    while getopts ":vn" opt; do
        case "$opt" in
            v)
                verbose=true
                ;;
            n)
                enable_venv=false
                ;;
            :)
                echo "Error: -$OPTARG requires an argument" >&2
                exit 1
                ;;
            \?)
                echo "Error: invalid option -$OPTARG" >&2
                nizam_help >&2
                exit 1
                ;;
        esac
    done

    if [[ "$verbose" == true ]]; then
        set -x
    fi

    # preflight checks
    command -v python3 >/dev/null 2>&1 || { echo "Error: python3 not found" >&2; exit 1; }
    command -v cargo >/dev/null 2>&1 || { echo "Error: cargo not found (install rustup)" >&2; exit 1; }
    command -v rustc >/dev/null 2>&1 || { echo "Error: rustc not found (install rustup)" >&2; exit 1; }

    local py_bin="python3"
    local pip_bin="pip3"

    if [[ "$enable_venv" == true ]]; then
        if [[ ! -d .venv ]]; then
            echo "[nizam] Creating virtual environment..."
            "$py_bin" -m venv .venv
        fi

        # shellcheck disable=SC1091
        source .venv/bin/activate
        py_bin="$(command -v python)"
        pip_bin="$(command -v pip)"

        echo "[nizam] Ensuring pip is up to date..."
        "$pip_bin" install -q --upgrade pip >/dev/null 2>&1 || true

        if [[ ! -x ".venv/bin/maturin" ]]; then
            echo "[nizam] Installing maturin..."
            "$pip_bin" install "maturin>=1.5,<2.0"
        else
            echo "[nizam] Using existing maturin in .venv"
        fi
    else
        if [[ ! -d .venv || ! -f .venv/bin/activate ]]; then
            echo "Error: .venv not found. Run without -n first to initialize it." >&2
            exit 1
        fi

        echo "[nizam] Using existing virtual environment (-n)"
        # shellcheck disable=SC1091
        source .venv/bin/activate
        py_bin="$(command -v python)"
        pip_bin="$(command -v pip)"

        if [[ ! -x ".venv/bin/maturin" ]]; then
            echo "[nizam] maturin not found in .venv, installing..."
            "$pip_bin" install "maturin>=1.5,<2.0"
        fi
    fi

    # handle pyo3 0.23 + python 3.14 incompatibility (abi3 forward compat)
    local py_ver
    py_ver="$("$py_bin" -c 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")')"
    if [[ "$py_ver" =~ ^3\.1[4-9] ]]; then
        echo "[nizam] Python $py_ver detected, enabling PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1"
        export PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
    fi

    echo "[nizam] Building extension with maturin develop..."
    maturin develop

    echo "[nizam] Running main.py..."
    "$py_bin" ./nizam
}

function main() {
    case "$option" in
        test)
            nizam_check "$@"
            ;;
        run)
            nizam_run "$@"
            ;;
        help|-h|--help)
            nizam_help
            ;;
        *)
            echo "Error: unknown subcommand '$option'" >&2
            nizam_help >&2
            exit 1
            ;;
    esac
}

main
