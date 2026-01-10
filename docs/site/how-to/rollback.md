# How to Rollback Changes

System Manager keeps track of previous configurations as "generations." If something goes wrong after applying a new configuration, you can rollback to a previous working state.

## List Available Generations

See all available generations:

```bash
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --list-generations
```

## Rollback to the Previous Generation

Quickly revert to the last working configuration:

```bash
# Switch to the previous generation
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --rollback

# Activate the rollback
nix run 'github:numtide/system-manager' -- activate --sudo
```

## Rollback to a Specific Generation

To rollback to a specific generation number:

```bash
# Switch to generation 42
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --switch-generation 42

# Activate it
nix run 'github:numtide/system-manager' -- activate --sudo
```

## Cleaning Up Old Generations

Over time, old generations consume disk space. Remove them with:

```bash
# Remove all old generations (keeps current)
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --delete-generations old

# Run garbage collection to reclaim space
sudo nix-collect-garbage -d
```

## Keeping Recent Generations

To keep the last N generations instead of deleting all old ones, use `+N`. For example, to keep the last 5 generations:

```bash
sudo nix-env --profile /nix/var/nix/profiles/system-manager-profiles --delete-generations +5
```

## See Also

- [Getting Started](../tutorials/getting-started.md) - Initial setup
- [CLI Reference](../reference/cli.md) - Full command documentation
