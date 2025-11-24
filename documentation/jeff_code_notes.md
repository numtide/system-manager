THIS FILE WILL GET REMOVED LATER! IT'S INTERNAL NOTES FOR JEFFREY

main.rs

The main.rs file is the entry point for the System Manager CLI. It defines the full command layout using clap and routes each subcommand to the right part of the codebase. The main subcommands are switch (build, register, and activate), register (build and register), build (just build), deactivate (remove everything System Manager manages), prepopulate (write files without starting services), and activate (activate from an already-built path).

The file handles both local and remote execution. If --target-host is provided, System Manager connects over SSH, optionally using sudo if --use-remote-sudo is set. For local use, it checks that the user has the necessary permissions before making system changes.

The high-level workflow looks like this: parse the CLI arguments, decide whether the operation is local or remote, build the Nix derivations from the provided flake URI, copy the result to the remote host if needed, register profiles using nix-env, and finally call into the activation logic. Error handling is centralized through handle_toplevel_error, which reports issues cleanly and ensures consistent exit codes. The file also includes helpers for resolving Nix store paths and falling back to the currently active profile when no explicit path is given. Any user-provided --nix-option flags are collected and passed along to the underlying Nix commands so that lower-level operations receive the same configuration.

lib.rs

The lib.rs file exposes the core library pieces used throughout System Manager. It re-exports the activate and register modules and defines the key paths System Manager uses for its internal state, including where profiles live, where GC roots are stored, and where the state directory resides on disk.

One important type defined here is StorePath, a small wrapper around PathBuf that guarantees the path points into the Nix store. It follows symlinks until it reaches a real store path and implements the usual conversions (From, TryFrom, Display, and serde traits) so it fits naturally into the rest of the code.

The file also includes NixOptions, a small struct for collecting --option flags that will later be passed to Nix commands. Several helper functions support filesystem work--creating and removing symlinks and directories, logging each action so it's easy to see what System Manager is doing. The etc_dir helper chooses between /etc and /run/etc depending on whether the tool is running in ephemeral mode, which is useful for container environments. Overall, lib.rs holds the shared types and utility functions that the rest of the code depends on.

activate.rs

The activate.rs file contains the logic that actually applies a System Manager configuration to the system. It defines the State structure, which tracks all managed files and services and persists that information as JSON so future activations know what is already present. Errors are represented by a custom ActivationError type, which makes it possible to record partial progress even when something goes wrong.

The main activate function coordinates the whole process: it runs pre-activation checks, writes and updates files under /etc, processes tmpfiles rules, and starts or reloads any systemd services defined in the configuration. The lighter prepopulate function performs only the file-related steps--useful in build environments where services shouldn't be started. The deactivate function performs a full teardown by stopping all services managed by System Manager and cleaning up every file it created, effectively restoring the system to how it looked before activation.

Throughout activation, System Manager logs each step and updates the state file at the end, even when errors occur. Pre-activation assertions are run as executable scripts sourced from the Nix store, allowing configuration authors to validate conditions before applying changes. The end result is a predictable, recoverable activation process that leaves a clear record of everything System Manager is responsible for.
