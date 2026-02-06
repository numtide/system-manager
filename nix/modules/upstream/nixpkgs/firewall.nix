# Mock of nixos/modules/services/networking/firewall.nix
#
# Defines networking.firewall options so NixOS service modules that open
# firewall ports (e.g. networking.firewall.allowedTCPPorts) can be imported
# by system-manager without evaluation errors.
#
# Emits a warning when port options are configured, since system-manager
# does not manage firewall rules on the host.
{ config, lib, ... }:
let
  cfg = config.networking.firewall;

  hasPortConfig =
    cfg.allowedTCPPorts != [ ]
    || cfg.allowedTCPPortRanges != [ ]
    || cfg.allowedUDPPorts != [ ]
    || cfg.allowedUDPPortRanges != [ ]
    || lib.any (
      iface:
      iface.allowedTCPPorts != [ ]
      || iface.allowedTCPPortRanges != [ ]
      || iface.allowedUDPPorts != [ ]
      || iface.allowedUDPPortRanges != [ ]
    ) (lib.attrValues cfg.interfaces);

  formatPorts = ports: lib.concatMapStringsSep ", " toString ports;
  formatRanges =
    ranges: lib.concatMapStringsSep ", " (r: "${toString r.from}-${toString r.to}") ranges;

  portSummary = lib.concatStringsSep "" (
    lib.optional (cfg.allowedTCPPorts != [ ]) "\n  TCP: ${formatPorts cfg.allowedTCPPorts}"
    ++ lib.optional (
      cfg.allowedTCPPortRanges != [ ]
    ) "\n  TCP ranges: ${formatRanges cfg.allowedTCPPortRanges}"
    ++ lib.optional (cfg.allowedUDPPorts != [ ]) "\n  UDP: ${formatPorts cfg.allowedUDPPorts}"
    ++ lib.optional (
      cfg.allowedUDPPortRanges != [ ]
    ) "\n  UDP ranges: ${formatRanges cfg.allowedUDPPortRanges}"
  );
in
let
  canonicalizePortList = ports: lib.unique (builtins.sort builtins.lessThan ports);

  commonOptions = {
    allowedTCPPorts = lib.mkOption {
      type = lib.types.listOf lib.types.port;
      default = [ ];
      apply = canonicalizePortList;
      example = [
        22
        80
      ];
      description = ''
        List of TCP ports on which incoming connections are accepted.
      '';
    };

    allowedTCPPortRanges = lib.mkOption {
      type = lib.types.listOf (lib.types.attrsOf lib.types.port);
      default = [ ];
      example = [
        {
          from = 8999;
          to = 9003;
        }
      ];
      description = ''
        A range of TCP ports on which incoming connections are accepted.
      '';
    };

    allowedUDPPorts = lib.mkOption {
      type = lib.types.listOf lib.types.port;
      default = [ ];
      apply = canonicalizePortList;
      example = [ 53 ];
      description = ''
        List of open UDP ports.
      '';
    };

    allowedUDPPortRanges = lib.mkOption {
      type = lib.types.listOf (lib.types.attrsOf lib.types.port);
      default = [ ];
      example = [
        {
          from = 60000;
          to = 61000;
        }
      ];
      description = ''
        Range of open UDP ports.
      '';
    };
  };
in
{
  options.networking.firewall = {
    enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to enable the firewall.
        Defaults to false in system-manager since firewall rules are managed
        by the host distribution.
      '';
    };

    logRefusedConnections = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to log rejected or dropped incoming connections.
      '';
    };

    logRefusedPackets = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to log all rejected or dropped incoming packets.
      '';
    };

    logRefusedUnicastsOnly = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        If logRefusedPackets is enabled, only log unicast packets.
      '';
    };

    rejectPackets = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        If set, refused packets are rejected rather than dropped.
      '';
    };

    trustedInterfaces = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "enp0s2" ];
      description = ''
        Traffic coming in from these interfaces will be accepted unconditionally.
      '';
    };

    allowPing = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Whether to respond to incoming ICMPv4 echo requests.
      '';
    };

    pingLimit = lib.mkOption {
      type = lib.types.nullOr (lib.types.separatedString " ");
      default = null;
      description = ''
        If pings are allowed, this allows setting rate limits on them.
      '';
    };

    checkReversePath = lib.mkOption {
      type = lib.types.either lib.types.bool (
        lib.types.enum [
          "strict"
          "loose"
        ]
      );
      default = true;
      description = ''
        Performs a reverse path filter test on a packet.
      '';
    };

    logReversePathDrops = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Logs dropped packets failing the reverse path filter test.
      '';
    };

    filterForward = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Enable filtering in IP forwarding.
      '';
    };

    connectionTrackingModules = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        List of connection-tracking helpers that are auto-loaded.
      '';
    };

    autoLoadConntrackHelpers = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Whether to auto-load connection-tracking helpers.
      '';
    };

    extraPackages = lib.mkOption {
      type = lib.types.listOf lib.types.package;
      default = [ ];
      description = ''
        Additional packages to be included in the environment of the system.
      '';
    };

    interfaces = lib.mkOption {
      default = { };
      type = lib.types.attrsOf (lib.types.submodule [ { options = commonOptions; } ]);
      description = ''
        Interface-specific open ports.
      '';
    };

    allInterfaces = lib.mkOption {
      internal = true;
      visible = false;
      default = { };
      type = lib.types.attrsOf (lib.types.submodule [ { options = commonOptions; } ]);
      description = ''
        All open ports.
      '';
    };
  }
  // commonOptions;

  config.warnings = lib.optional hasPortConfig ''
    Firewall port configurations are set but will not be applied.
    system-manager does not manage firewall rules on the host.
    Ensure these ports are opened in your host firewall:${portSummary}
  '';
}
