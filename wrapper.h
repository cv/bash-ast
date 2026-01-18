/* wrapper.h -- Minimal header for bindgen to generate FFI bindings to bash */

/* System headers needed for basic types */
#include <sys/types.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

/* We need config.h first for all the feature flags */
#include "config.h"

/* Core command structures - must come before general.h */
#include "command.h"

/* Include general.h for GENERIC_LIST and other typedefs */
#include "general.h"

/* Dispose functions */
#include "dispose_cmd.h"

/* Parser functions */
#include "externs.h"

/* Global variables we need - declare them extern */
extern int interactive;
extern int interactive_shell;
extern int login_shell;
extern int posixly_correct;
extern int shell_initialized;
extern int startup_state;
extern int parsing_command;

/* Safe parse wrappers - use SX_NOLONGJMP to prevent crashes on syntax errors */
extern COMMAND *safe_parse_string_to_command(char *string, int flags);
extern COMMAND *safe_parse_verbose(char *string, int flags);

/* Parse a complete multi-command script */
extern COMMAND *safe_parse_script(char *string, int flags);

/* Initialization functions we need to call */
extern void initialize_shell_builtins(void);
extern void initialize_traps(void);
extern void initialize_signals(int on_or_off);
extern void initialize_shell_variables(char **env, int privmode);
extern int initialize_job_control(int forced);
extern void initialize_bash_input(void);
extern void initialize_flags(void);
