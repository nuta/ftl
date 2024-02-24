#!/bin/bash
set -ue -o pipefail

make_args=()
for arg in "$@"; do
  case $arg in
    --help|-h)
      echo "Usage: $0 [VAR=value...]"
      exit 0
      ;;
    # Override environment variables: ./x MAKE=...
    #
    # Of course you can also use `MAKE=... ./x` to do the same thing, but this
    # is consistent with make's syntax.
    *=*)
      name=${arg%%=*}
      value=${arg#*=}
      if ! [[ $name =~ ^[a-zA-Z_][a-zA-Z0-9_]*$ ]]; then
        echo "Invalid variable name: $name" >&2
        exit 1
      fi

      export $name="$value"
      ;;
    *)
      make_args+=("$arg")
      ;;
  esac
done

FTL_DIR="${FTL_DIR:-.}"
MAKE="${MAKE:-make}"

$MAKE -C "$FTL_DIR" "${make_args[@]}"
