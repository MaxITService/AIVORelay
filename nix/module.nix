# NixOS module for AivoRelay speech-to-text
#
# Handles system-level configuration that the package wrapper cannot:
#   - udev rule for /dev/uinput (rdev grab() needs it for virtual input)
#
# Note: users must add themselves to the "input" group for evdev hotkey access.
#
# Usage in your flake:
#
#   inputs.aivorelay.url = "github:MaxITService/AIVORelay";
#
#   nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
#     modules = [
#       aivorelay.nixosModules.default
#       { programs.aivorelay.enable = true; }
#     ];
#   };
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.aivorelay;
in
{
  options.programs.aivorelay = {
    enable = lib.mkEnableOption "AivoRelay offline speech-to-text";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "aivorelay.packages.\${system}.aivorelay";
      description = "The AivoRelay package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # rdev grab() creates virtual input devices via /dev/uinput.
    # Default permissions are crw------- root root — open it to the input group.
    services.udev.extraRules = ''
      KERNEL=="uinput", GROUP="input", MODE="0660"
    '';
  };
}
