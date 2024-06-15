# thanks getchoo :3
self: {
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.api-rs;

  inherit (pkgs.stdenv.hostPlatform) system;

  inherit
    (lib)
    getExe
    literalExpression
    mdDoc
    mkDefault
    mkEnableOption
    mkIf
    mkOption
    mkPackageOption
    types
    ;
in {
  options.services.api-rs = {
    enable = mkEnableOption "api-rs";
    package = mkPackageOption self.packages.${system} "api-rs" {};
    ports = mkOption {
      type = types.listOf types.int;
      default = [5000];
    };
    environmentFile = mkOption {
      description = mdDoc ''
        Environment file as defined in {manpage}`systemd.exec(5)`
      '';
      type = types.nullOr types.path;
      default = null;
      example = literalExpression ''
        "/run/agenix.d/1/api-rs"
      '';
    };
  };

  config = mkIf cfg.enable {
    networking.firewall.allowedTCPPorts = cfg.ports;

    systemd.services."api-rs" = {
      enable = true;
      wantedBy = mkDefault ["multi-user.target"];
      wants = mkDefault ["network-online.target"];
      after = mkDefault ["network.target" "network-online.target"];
      script = ''
        ${getExe cfg.package}
      '';

      serviceConfig = {
        Type = "simple";
        Restart = "always";
        RestartSec = "5s";

        EnvironmentFile = mkIf (cfg.environmentFile != null) cfg.environmentFile;

        # hardening
        DynamicUser = true;
        PrivateTmp = true;
        NoNewPrivileges = true;
        RestrictNamespaces = "uts ipc pid user cgroup";
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        PrivateDevices = true;
        RestrictSUIDSGID = true;
      };
    };
  };
}
