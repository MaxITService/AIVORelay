# Home-manager module for AivoRelay speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ aivorelay.homeManagerModules.default ];
#        services.aivorelay.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.aivorelay;
in
{
  options.services.aivorelay = {
    enable = lib.mkEnableOption "AivoRelay speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "aivorelay.packages.\${system}.aivorelay";
      description = "The AivoRelay package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.aivorelay = {
      Unit = {
        Description = "AivoRelay speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/aivorelay";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
