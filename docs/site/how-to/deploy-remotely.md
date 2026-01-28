# How to Deploy to Remote Machines

System Manager can deploy configurations to remote machines via SSH. This guide covers deploying to one or more remote hosts.

## Prerequisites

- SSH access to the target machine
- Nix installed on the target machine
- Your SSH key configured for passwordless authentication (recommended)

Before you can deploy to a remote system, the remote machine's Nix daemon needs to trust incoming store paths from your local machine. Without this, the remote will reject the files you're trying to copy because they lack a signature from a trusted cache.

Edit `/etc/nix/nix.conf` on the remote system and ensure these lines are present; you'll need to add the appropriate usernames, such as "ubuntu" if you're using an Amazon EC2 Ubuntu server:

```
trusted-users = root ubuntu
build-users-group = nixbld
```

Then restart the Nix daemon:
```bash
sudo systemctl restart nix-daemon
```

## Basic Remote Deployment

Deploy your local configuration to a remote machine:

```bash
nix run 'github:numtide/system-manager' -- --target-host user@remote-host \
  switch \
  --flake . \
  --sudo
```

## Using a Remote Flake

Deploy a configuration hosted on GitHub directly to a remote machine:

```bash
nix run 'github:numtide/system-manager' -- --target-host user@remote-host \
  switch \
  --flake github:your-username/your-config \
  --sudo
```

## Using Different Configurations per Host

If you have different configurations for different hosts, use the flake output name:

```bash
# Deploy the "webserver" configuration to web.example.com
nix run 'github:numtide/system-manager' -- --target-host admin@web.example.com \
  switch \
  --flake .#webserver \
  --sudo

# Deploy the "database" configuration to db.example.com
nix run 'github:numtide/system-manager' -- --target-host admin@db.example.com \
  switch \
  --flake .#database \
  --sudo
```

## SSH Configuration Tips

To connect successfully, you have a couple of options for handling SSH authentication.

### Option 1: SSH Config Entry

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
nix run 'github:numtide/system-manager' -- --target-host webserver \
switch \
  --flake . \
  --sudo
```

### Option 2: SSH Agent

If you prefer not to modify your SSH config, you can load your key into the SSH agent before running System Manager:
```bash
eval $(ssh-agent -s)
ssh-add /home/ubuntu/.ssh/my.pem
nix run github:numtide/system-manager -- --target-host 'ubuntu@172.31.40.14' switch --flake . --sudo
```

This approach is particularly useful for automation scripts, as we'll see in the next section.

!!! Warning "Unsupported Format"
    You might be tempted to use Nix's query parameter syntax for SSH keys:
    
    `'ubuntu@172.31.40.14?ssh-key=/home/ubuntu/.ssh/my.pem'`
    
    Although Nix supports this format for some operations, System Manager does not. It will attempt to connect to the entire string literally as a hostname, resulting in a "Could not resolve hostname" error.

## Deploying to Multiple Systems

One of the major benefits of System Manager is the ability to manage entire fleets of machines with consistent configurations. Rather than manually connecting to each server, you can script deployments to dozens or even hundreds of systems.

Here's a basic example that deploys to multiple systems:
```bash
#!/bin/bash
set -e

# Start the SSH agent and add the key
eval $(ssh-agent -s)
ssh-add ~/.ssh/my.pem

# List of target hosts (using private IP addresses in this example)
HOSTS=(
    "ubuntu@172.31.40.14"
    "ubuntu@172.31.28.157"
    "ubuntu@172.31.25.67"
)

# Deploy to each host
for host in "${HOSTS[@]}"; do
    echo "Deploying to $host..."
    nix run github:numtide/system-manager -- --target-host "$host" switch --flake . --sudo
done

# Clean up the SSH agent
ssh-agent -k
echo "Deployment complete!"
```

### Reading Hosts from a File

For larger deployments, or when you want to separate your host inventory from your scripts, you can read the target addresses from an external file:
```bash
#!/bin/bash
set -e

eval $(ssh-agent -s)
ssh-add ~/.ssh/my.pem

while IFS= read -r host; do
    [[ -z "$host" || "$host" =~ ^# ]] && continue  # Skip empty lines and comments
    echo "Deploying to $host..."
    nix run github:numtide/system-manager -- --target-host "$host" switch --flake . --sudo
done < hosts.txt

ssh-agent -k
```

Where `hosts.txt` looks like:
```bash
# Production servers
ubuntu@172.31.40.14
ubuntu@172.31.40.15
# Staging
ubuntu@172.31.40.20
```

This makes it easy to maintain different host lists for different environments, or to generate the list dynamically from your infrastructure tooling.


## Handling SSH Host Key Verification

If this is the first time you've connected to the remote systems via SSH, you'll encounter the familiar host key verification prompt:
```
The authenticity of host '172.31.25.67 (172.31.25.67)' can't be established.
ED25519 key fingerprint is SHA256:rfG7uEi06BL+A2qXa+MWRx/k3JCZCyDjfUcpSBQ1zBI.
Are you sure you want to continue connecting (yes/no/[fingerprint])?
```

This will interrupt automated scripts. There are some different approaches to handle this.

### Approach 1: Pre-scan Host Keys

You can scan and add all host keys to your `known_hosts` file before deploying. This is explicit and works regardless of your SSH configuration:
```bash
#!/bin/bash
set -e

eval $(ssh-agent -s)
ssh-add ~/.ssh/my.pem

# Target hosts
HOSTS=(
    "ubuntu@172.31.40.14"
    "ubuntu@172.31.28.157"
    "ubuntu@172.31.25.67"
)

# Pre-scan and trust all host keys
echo "Scanning host keys..."
for host in "${HOSTS[@]}"; do
    ip="${host#*@}"  # Extract IP from ubuntu@172.31.x.x
    ssh-keyscan -H "$ip" >> ~/.ssh/known_hosts 2>/dev/null
done
echo "Host keys added to known_hosts"

# Deploy to each host
for host in "${HOSTS[@]}"; do
    echo "Deploying to $host..."
    nix run github:numtide/system-manager -- --target-host "$host" switch --flake . --sudo
done

ssh-agent -k
echo "Deployment complete!"
```

### Approach 2: Wildcard SSH Config

If your servers are all within a predictable subnet (common in cloud environments like AWS VPCs), you can use a wildcard pattern in your SSH config to handle authentication and host key verification automatically:

Add this to `~/.ssh/config`:
```
Host 172.31.*
    User ubuntu
    IdentityFile ~/.ssh/my.pem
    StrictHostKeyChecking accept-new
```

!!! Note
    `StrictHostKeyChecking accept-new` automatically trusts and saves host keys for servers you've never connected to before, without asking for confirmation. However, it will still warn you if a previously-saved key changes, which could indicate a security issue or a reinstalled server.

With this configuration in place, your deployment script becomes much simpler—no agent setup or key scanning required:
```bash
#!/bin/bash
set -e

# Target hosts - just the IPs, credentials come from SSH config
HOSTS=(
    "172.31.40.14"
    "172.31.28.157"
    "172.31.25.67"
)

# Deploy to each host
for host in "${HOSTS[@]}"; do
    echo "Deploying to $host..."
    nix run github:numtide/system-manager -- --target-host "$host" switch --flake . --sudo
done

echo "Deployment complete!"
```

!!! Tip
    The pre-scanning approach works well if you want to keep everything in a single self-contained script, which is ideal for CI/CD pipelines and automation systems where you may not have control over the SSH config.


## Deploying from a Remote Flake

So far, all examples have used `--flake .` to reference configuration files on your local machine. But you can also host your Nix configuration in a remote Git repository and deploy directly from there. This is powerful for CI/CD workflows where the configuration lives in version control and deployments are triggered automatically.

Simply replace the `.` with a flake URL pointing to your repository:
```bash
#!/bin/bash
set -e

eval $(ssh-agent -s)
ssh-add ~/.ssh/my.pem

# Target hosts
HOSTS=(
    "ubuntu@172.31.40.14"
    "ubuntu@172.31.28.157"
    "ubuntu@172.31.25.67"
)

# Pre-scan and trust all host keys
echo "Scanning host keys..."
for host in "${HOSTS[@]}"; do
    ip="${host#*@}"  # Extract IP from ubuntu@172.31.x.x
    ssh-keyscan -H "$ip" >> ~/.ssh/known_hosts 2>/dev/null
done
echo "Host keys added to known_hosts"

# Deploy to each host from a remote flake
for host in "${HOSTS[@]}"; do
    echo "Deploying to $host..."
    nix run github:numtide/system-manager -- --target-host "$host" switch --flake git+https://github.com/numtide/system-manager-test#default --sudo
done

ssh-agent -k
echo "Deployment complete!"
```

!!! Warning "Keep Your Flake Lock Updated"
    When using remote flakes, make sure the repository's `flake.lock` file references a compatible version of System Manager. If the lock file points to an older version, you may encounter errors about missing binaries like `system-manager-engine`. Run `nix flake update` in your repository to update the lock file to the latest version.

## See Also

- [Use Remote Flakes](use-remote-flakes.md) - Host your configuration on GitHub
- [CLI Reference](../reference/cli.md) - Full command documentation
