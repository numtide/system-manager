# Docker

This example shows how to install Docker and configure it as a systemd service.

## Configuration

```nix
{
  description = "System Manager - Docker Example";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, system-manager }: {
    systemConfigs.default = system-manager.lib.makeSystemConfig {
      modules = [
        {
          nixpkgs.hostPlatform = "x86_64-linux";

          # Install Docker and related tools
          environment.systemPackages = with nixpkgs.legacyPackages.x86_64-linux; [
            docker
            docker-compose
            docker-buildx
          ];

          # Docker daemon configuration
          environment.etc."docker/daemon.json".text = ''
            {
              "log-driver": "json-file",
              "log-opts": {
                "max-size": "10m",
                "max-file": "3"
              },
              "storage-driver": "overlay2",
              "storage-opts": [
                "overlay2.override_kernel_check=true"
              ]
            }
          '';

          # Create Docker systemd service
          systemd.services.docker = {
            enable = true;
            description = "Docker Application Container Engine";
            documentation = [ "https://docs.docker.com" ];
            after = [ "network-online.target" "firewalld.service" "containerd.service" ];
            wants = [ "network-online.target" ];
            requires = [ "docker.socket" ];
            wantedBy = [ "system-manager.target" ];

            serviceConfig = {
              Type = "notify";
              ExecStart = "${nixpkgs.legacyPackages.x86_64-linux.docker}/bin/dockerd --host=fd://";
              ExecReload = "/bin/kill -s HUP $MAINPID";
              TimeoutStartSec = 0;
              RestartSec = 2;
              Restart = "always";
              StartLimitBurst = 3;
              StartLimitInterval = "60s";

              # Security settings
              LimitNOFILE = 1048576;
              LimitNPROC = "infinity";
              LimitCORE = "infinity";
              TasksMax = "infinity";
              Delegate = "yes";
              KillMode = "process";
              OOMScoreAdjust = -500;
            };
          };

          # Docker socket
          systemd.sockets.docker = {
            enable = true;
            description = "Docker Socket for the API";
            wantedBy = [ "sockets.target" ];

            socketConfig = {
              ListenStream = "/var/run/docker.sock";
              SocketMode = "0660";
              SocketUser = "root";
              SocketGroup = "docker";
            };
          };

          # Create necessary directories and setup
          systemd.tmpfiles.rules = [
            "d /var/lib/docker 0710 root root -"
            "d /var/run/docker 0755 root root -"
            "d /etc/docker 0755 root root -"
          ];
        }
      ];
    };
  };
}
```

## Usage

```bash
# Activate the configuration
nix run 'github:numtide/system-manager' -- switch --flake /path/to/this/example --sudo

# Check Docker service status
sudo systemctl status docker

# Test Docker
sudo docker run hello-world

# Check Docker version
sudo docker --version

# View Docker logs
sudo journalctl -u docker -f
```

## Notes

- Ensure the `docker` group exists on your system
- Add your user to the docker group: `sudo usermod -aG docker $USER`
- You may need to log out and back in for group changes to take effect
- This example uses the Docker socket for API communication
