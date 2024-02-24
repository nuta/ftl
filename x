#!/bin/bash
set -ue -o pipefail

function abort() {
  echo "ftl: $@" >&2
  exit 1
}

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
        abort "invalid variable name: $name"
        exit 1
      fi

      export $name="$value"
      ;;
    *)
      make_args+=("$arg")
      ;;
  esac
done

if [[ ${#make_args[@]} -eq 0 ]]; then
  abort "no make target specified"
fi

FTL_DIR="${FTL_DIR:-.}"
MAKE="${MAKE:-make}"

$MAKE -C "$FTL_DIR" "${make_args[@]}"
