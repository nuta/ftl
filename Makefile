# The default build target.
.PHONY: default
default: all

# Disable builtin implicit rules and variables.
MAKEFLAGS += --no-builtin-rules --no-builtin-variables
.SUFFIXES:

# Enable verbose output if $(V) is set.
ifeq ($(V),)
.SILENT:
endif

PROGRESS   := printf "\\033[1;94m==>\\033[0m \\033[1m%s\\033[0m \\n"

default: ftl.elf
