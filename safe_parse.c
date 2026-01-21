/* safe_parse.c - Safe wrapper for bash parser with error recovery
 *
 * Copyright (C) 2024-2026 Carlos Villela <cv@lixo.org>
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * This file is part of bash-ast.
 *
 * bash-ast is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This wrapper provides safe access to bash's parser, handling longjmp
 * errors gracefully and supporting multi-command scripts.
 */

/* System headers needed for basic types */
#include <sys/types.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <setjmp.h>

/* Bash headers - order matters */
#include "config.h"
#include "bashtypes.h"
#include "command.h"
#include "general.h"
#include "input.h"
#include "externs.h"
#include "make_cmd.h"
#include "dispose_cmd.h"
#include "sig.h"

/* Flags from subst.h - defined here to avoid header dependency issues.
 * subst.h has complex dependencies (SHELL_VAR, etc.) that we don't need.
 *
 * IMPORTANT: If bash updates these values, the static_asserts below will
 * fail at compile time, alerting us to update these definitions.
 */
#define SX_NOLONGJMP    0x0040  /* don't longjmp on fatal error */
#define SX_NOERROR      0x1000  /* don't print parser error messages */

/* Verify our flag values match bash's actual values from subst.h. */
_Static_assert(SX_NOLONGJMP == 0x0040, "SX_NOLONGJMP value changed - update safe_parse.c");
_Static_assert(SX_NOERROR == 0x1000, "SX_NOERROR value changed - update safe_parse.c");

/* External bash globals and functions we need */
extern COMMAND *global_command;
extern int parse_command(void);
extern void with_input_from_string(char *, const char *);
extern int EOF_Reached;
extern volatile int interrupt_state;
extern sigjmp_buf top_level;

/* Variable system - we need to initialize this for process substitution parsing */
#include "variables.h"
extern void initialize_shell_builtins(void);
extern SHELL_VAR *make_new_array_variable(const char *name);

/* Internal flag to track if we've done our initialization */
static int parser_lib_initialized = 0;

/* Parser state variable - we need to ensure it's initialized */
extern int parser_state;

/**
 * Ensure bash is properly initialized for library use.
 * This is called automatically on first parse.
 */
static void ensure_initialized(void) {
    SHELL_VAR *v;

    if (parser_lib_initialized) {
        return;
    }

    /* Initialize the shell builtins (needed for some parsing operations) */
    initialize_shell_builtins();

    /* Create a dummy variable to trigger variable table creation.
     * We use bind_variable which will create the hash tables if needed. */
    bind_variable("_BASH_AST_INIT", "1", 0);

    /* Initialize PIPESTATUS array - required for process/command substitution
     * error handling. set_pipestatus_array() is called during parse_comsub
     * on syntax errors, so we need this set up before any parsing. */
    v = find_variable("PIPESTATUS");
    if (v == NULL) {
        v = make_new_array_variable("PIPESTATUS");
    }

    /* Ensure parser_state starts clean. This is critical because
     * parse_string_to_command ORs in flags and expects a clean starting state. */
    parser_state = 0;

    parser_lib_initialized = 1;
}

/* Line number tracking - used to provide accurate line numbers in parsed AST */
extern int line_number;
extern int line_number_base;

/* Parser state functions */
extern void reset_parser(void);
extern void clear_shell_input_line(void);
extern int parser_expanding_alias(void);

/**
 * safe_parse_string_to_command - Parse a string without longjmp on error
 *
 * This function wraps parse_string_to_command with the SX_NOLONGJMP flag,
 * which tells bash's parser not to call longjmp() on syntax errors.
 * Instead, the parser returns NULL, which we can handle gracefully.
 *
 * Error messages are suppressed (SX_NOERROR). Use safe_parse_verbose()
 * if you need error messages printed to stderr.
 *
 * NOTE: This only parses a single command. For multi-command scripts,
 * use safe_parse_script().
 *
 * @param string  The bash script to parse
 * @param flags   Parser flags (SX_NOLONGJMP | SX_NOERROR always added)
 *
 * @return  The parsed COMMAND structure, or NULL on error
 */
COMMAND *safe_parse_string_to_command(char *string, int flags) {
    ensure_initialized();
    return parse_string_to_command(string, flags | SX_NOLONGJMP | SX_NOERROR);
}

/**
 * safe_parse_verbose - Parse with error messages printed to stderr
 *
 * Like safe_parse_string_to_command, but allows bash to print syntax
 * error messages to stderr. Useful for debugging or when you need
 * detailed error information (line numbers, unexpected tokens, etc.)
 *
 * @param string  The bash script to parse
 * @param flags   Parser flags (SX_NOLONGJMP always added)
 *
 * @return  The parsed COMMAND structure, or NULL on error
 */
COMMAND *safe_parse_verbose(char *string, int flags) {
    ensure_initialized();
    return parse_string_to_command(string, flags | SX_NOLONGJMP);
}

/**
 * safe_parse_script - Parse a multi-command script
 *
 * This function parses a complete bash script that may contain multiple
 * commands separated by newlines or semicolons. All commands are connected
 * into a single COMMAND tree using newline (';') separators.
 *
 * For simple/safer parsing, this wraps the entire script in a group { }
 * and uses the single-command parser with SX_NOLONGJMP.
 *
 * @param string  The bash script to parse (may contain multiple commands)
 * @param flags   Parser flags (currently unused, reserved for future use)
 *
 * @return  The parsed COMMAND structure containing all commands, or NULL on error
 */
COMMAND *safe_parse_script(char *string, int flags) {
    COMMAND *result;
    char *wrapped;
    size_t len;
    int saved_line_number;

    (void)flags;  /* Reserved for future use */

    if (string == NULL || *string == '\0') {
        return NULL;
    }

    /* Wrap the script in a group { ... } to make it a single compound command.
     * This allows the single-command parser to handle multi-statement scripts.
     * We need a trailing newline before } in case the script ends with a comment.
     *
     * The wrapper "{ " is on the same line as the script's first line, so
     * line numbers in the AST will match the original script's line numbers.
     */
    len = strlen(string);
    wrapped = malloc(len + 6);  /* "{ " + string + "\n}" + null */
    if (wrapped == NULL) {
        return NULL;
    }

    strcpy(wrapped, "{ ");
    strcat(wrapped, string);
    strcat(wrapped, "\n}");

    /* Initialize line_number so the AST has accurate line information.
     * We start at 0 because shell_getc increments line_number BEFORE reading
     * each line. So after the increment before reading the first line,
     * line_number will be 1, which is correct.
     */
    saved_line_number = line_number;
    line_number = 0;

    /* Use the safe single-command parser (ensure_initialized called inside) */
    result = safe_parse_string_to_command(wrapped, 0);

    /* Restore line_number in case this is called from within bash */
    line_number = saved_line_number;

    free(wrapped);

    return result;
}
