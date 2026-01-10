# How to Deploy to Remote Machines

System Manager can deploy configurations to remote machines via SSH. This guide covers deploying to one or more remote hosts.

## Prerequisites

- SSH access to the target machine
- Nix installed on the target machine
- Your SSH key configured for passwordless authentication (recommended)

## Basic Remote Deployment

Deploy your local configuration to a remote machine:

```bash
nix run 'github:numtide/system-manager' -- switch \
  --flake . \
  --target-host user@remote-host \
  --sudo
```

## Using a Remote Flake

Deploy a configuration hosted on GitHub directly to a remote machine:

```bash
nix run 'github:numtide/system-manager' -- switch \
  --flake github:your-username/your-config \
  --target-host user@remote-host \
  --sudo
```

## Multiple Machines

For deploying to multiple machines, run the command for each host:

```bash
for host in server1 server2 server3; do
  nix run 'github:numtide/system-manager' -- switch \
    --flake . \
    --target-host admin@$host \
    --sudo
done
```

## Using Different Configurations per Host

If you have different configurations for different hosts, use the flake output name:

```bash
# Deploy the "webserver" configuration to web.example.com
nix run 'github:numtide/system-manager' -- switch \
  --flake .#webserver \
  --target-host admin@web.example.com \
  --sudo

# Deploy the "database" configuration to db.example.com
nix run 'github:numtide/system-manager' -- switch \
  --flake .#database \
  --target-host admin@db.example.com \
  --sudo
```

## SSH Configuration Tips

For easier deployment, add your hosts to `~/.ssh/config`:

```
Host webserver
  HostName web.example.com
  User admin
  IdentityFile ~/.ssh/deploy_key

Host dbserver
  HostName db.example.com
  User admin
  IdentityFile ~/.ssh/deploy_key
```

Then deploy with just:

```bash
nix run 'github:numtide/system-manager' -- switch \
  --flake . \
  --target-host webserver \
  --sudo
```

## See Also

- [Use Remote Flakes](use-remote-flakes.md) - Host your configuration on GitHub
- [CLI Reference](../reference/cli.md) - Full command documentation
