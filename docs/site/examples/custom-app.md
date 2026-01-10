# Custom App

This example shows how to deploy custom web software from a repository and run it behind Nginx.

## Live example

We have a complete example live that you can try out. All you need is a fresh server (such as on Amazon EC2) with at least 16GB memory. (We recommend the latest Ubuntu, with a t3Large instance, with 16GB RAM. Then allow SSH, HTTP traffic, and HTTPS traffic if you plan to build on these examples.) We have two repos:

1. The sample application

2. The configuration files

The configuration files install both nginx and the sample app.

After you spin up an instance, install nix for all users:

```sh
sh <(curl --proto '=https' --tlsv1.2 -L https://nixos.org/nix/install) --daemon
```

Next, log out and log back in so that nix is available in the system path.

And then you can run System Manager and deploy the app with one command:

```sh
nix run 'github:numtide/system-manager' --extra-experimental-features 'nix-command flakes' -- switch --flake github:frecklefacelabs/system-manager-custom-app-deploy/v1.0.0#default --sudo
```

(Remember, the first time System Manager runs, it takes up to five minutes or so to compile everything.)

!!! Tip
    We're specifying a tag in our URL. This is good practice to make sure you get the right version of your flakes. Also, modern Nix supports the use of a protocol called "github", and when you use that protocol, you can specify the tag behind a slash symbol, as we did here for tag v1.0.0.

!!! Tip
    If you make changes to your flakes, be sure to create a new tag. Without it, Nix sometimes refuses to load the "latest version" of the repo, and will insist on using whatever version of your repo it used first.

Then, the app should be installed, with nginx sitting in front of it, and you should be able to run:

```sh
curl localhost
```
And it will print out a friendly JSON message such as:

```json
{"message":"Welcome to the Bun API!","status":"running","endpoints":["/","/health","/random","/cowsay"]}
```

We even included cowsay in this sample, which you can try at `curl localhost/cowsay`. Now even though cowsay is meant for fun, the primary reason is this is a TypeScript app that uses `bun`, and we wanted to demonstrate how easy it is to include `npm` libraries. `bun` includes a feature whereby it will install dependency packages from `package.json` automatically the first time it runs, greatly simplifying the setup.

One thing about the `.nix` files in this repo is that they in turn pull code (our TypeScript app) from another remote repo. Using this approach, you can separate concerns, placing the deployment `.nix` files in one repo, and the source app in a separate repo.

## Configuration files

Here are further details on the individual `.nix` files.

### flake.nix

First we have a flake much like the usual starting point:

```nix
# flake.nix
{
  description = "Standalone System Manager configuration";

  inputs = {
    # Specify the source of System Manager and Nixpkgs.
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    system-manager = {
      url = "github:numtide/system-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      system-manager,
      ...
    }:
    let
      system = "x86_64-linux";
    in
    {
      systemConfigs.default = system-manager.lib.makeSystemConfig {

        # Specify your system configuration modules here, for example,
        # the path to your system.nix.
        modules = [

          {
            nix.settings.experimental-features = "nix-command flakes";
            services.myapp.enable = true;
          }
            ./system.nix
            ./nginx.nix
            ./bun-app.nix
        ];

        # Optionally specify extraSpecialArgs and overlays
      };
    };
}
```

### nginx.nix

Next is the `.nix` configuration that installs and configures nginx. This is a simple nginx configuration, as it simply routes incoming HTTP traffic directly to the app:

```nix
# nginx.nix
{ config, lib, pkgs, ... }:
{
  config = {
    services.nginx = {
      enable = true;

      recommendedGzipSettings = true;
      recommendedOptimisation = true;
      recommendedProxySettings = true;
      recommendedTlsSettings = true;

      virtualHosts."_" = {
        default = true;

        locations."/" = {
          proxyPass = "http://127.0.0.1:3000";
          extraConfig = ''
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
          '';
        };

        locations."/health" = {
          proxyPass = "http://127.0.0.1:3000/health";
          extraConfig = ''
            access_log off;
          '';
        };
      };
    };
  };
}
```

### bun-app.nix

Next, here's the `.nix` configuration that creates a service that runs the app.

```nix
# bun-app.nix
{ config, lib, pkgs, ... }:
let
  # Fetch the app from GitHub
  appSource = pkgs.fetchFromGitHub {
    owner = "frecklefacelabs";
    repo = "typescript_app_for_system_manager";
    rev = "v1.0.0";  # Use a tag
    sha256 = "sha256-TWt/Y2B7cGxjB9pxMOApt83P29uiCBv5nVT3KyycYEA=";
  };
in
{
  config = {
    nixpkgs.hostPlatform = "x86_64-linux";

    # Install Bun
    environment.systemPackages = with pkgs; [
      bun
    ];

    # Simple systemd service - runs Bun directly from Nix store!
    systemd.services.bunapp = {
      description = "Bun TypeScript Application";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        User = "ubuntu";
        Group = "ubuntu";
        WorkingDirectory = "${appSource}";
        # Bun will auto-install dependencies from package.json on first run
        ExecStart = "${pkgs.bun}/bin/bun run index.ts";
        Restart = "always";
        RestartSec = "10s";
      };

      environment = {
        NODE_ENV = "production";
      };
    };
  };
}

```

### index.ts (The application)

And finally, here's the `index.ts` file; it's just a simple REST app that also makes use of one third-party `npm` library.

```typescript
import cowsay from "cowsay";

const messages = [
  "Hello from System Manager!",
  "Bun is blazingly fast!",
  "Nix + Bun = Easy deployments",
  "Making it happen!",
  "Nix rocks!"
];

const server = Bun.serve({
  port: 3000,
  fetch(req) {
    const url = new URL(req.url);

    if (url.pathname === "/") {
      return new Response(JSON.stringify({
        message: "Welcome to the Bun API!",
        status: "running",
        endpoints: ["/", "/health", "/random", "/cowsay"]
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }

    if (url.pathname === "/health") {
      return new Response(JSON.stringify({
        status: "healthy"
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }

    if (url.pathname === "/random") {
      const randomMessage = messages[Math.floor(Math.random() * messages.length)];
      return new Response(JSON.stringify({
        message: randomMessage,
        timestamp: new Date().toISOString()
      }), {
        headers: { "Content-Type": "application/json" }
      });
    }

    if (url.pathname === "/cowsay") {
      const cow = cowsay.say({
        text: "Deployed with System Manager and Nix!"
      });
      return new Response(cow, {
        headers: { "Content-Type": "text/plain" }
      });
    }

    return new Response("Not Found", { status: 404 });
  },
});

console.log(`Server running on http://localhost:${server.port}`);
```

## What this configuration does

1. **Fetches the application** from GitHub using `pkgs.fetchFromGitHub`
2. **Installs Bun** as a system package
3. **Creates a systemd service** that:
   - Runs the TypeScript app using Bun
   - Automatically restarts on failure
   - Sets the working directory to the fetched source
4. **Configures Nginx** as a reverse proxy to the app on port 3000

## Key concepts

- **Separation of concerns**: The deployment configuration (`.nix` files) lives in one repo, while the application source lives in another
- **Automatic dependency installation**: Bun installs npm dependencies from `package.json` on first run
- **Reproducible deployments**: The `sha256` hash ensures you get the exact version you expect
