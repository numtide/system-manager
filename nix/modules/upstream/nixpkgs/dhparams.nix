{ config, lib, ... }:
let
  cfg = config.security.dhparams;
in
{
  config = lib.mkIf (cfg.enable && cfg.stateful) {
  };
}
