{
  config,
  lib,
  pkgs,
  ...
}:

with lib;

let
  cfg = config.services.colorctl;
in
{
  options.services.colorctl = {
    enable = mkEnableOption "Colorful motherboard RGB lighting and fan curve management";

    package = mkOption {
      type = types.package;
      default = pkgs.colorctl;
      description = "The colorctl package to use.";
    };

    fan = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Apply fan curve settings automatically on boot and resume.";
      };

      profile = mkOption {
        type = types.enum [
          "quiet"
          "standard"
          "full"
        ];
        default = "quiet";
        description = "Built-in fan curve profile (quiet, standard, full).";
      };

      customPoints = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "30:25,50:45,70:75,85:100";
        description = "Custom 4-point curve in format 'T1:P1,T2:P2,T3:P3,T4:P4'. Overrides profile if specified.";
      };

      header = mkOption {
        type = types.str;
        default = "all";
        description = "Target fan header ('all', 'cpu', 'cha_fan1', 'cha_fan2', 'cha_fan3', 'pump').";
      };
    };

    rgb = {
      enable = mkOption {
        type = types.bool;
        default = true;
        description = "Apply RGB lighting settings automatically on boot.";
      };

      color = mkOption {
        type = types.str;
        default = "cyan";
        example = "#ff0088";
        description = "Color to set on boot (color name or hex).";
      };

      brightness = mkOption {
        type = types.ints.between 0 100;
        default = 100;
        description = "RGB brightness level (0-100%).";
      };

      channel = mkOption {
        type = types.str;
        default = "all";
        description = "Target RGB channel ('all', 'led', '12v-1', '12v-2', '5v-1', '5v-2', '5v-3').";
      };
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # Udev rule allowing unprivileged users to control RGB via /dev/hidraw
    services.udev.extraRules = ''
      KERNEL=="hidraw*", ATTRS{idVendor}=="2f4c", ATTRS{idProduct}=="1000", MODE="0666", TAG+="uaccess"
    '';

    # Restart service when waking up from suspend/hibernate
    powerManagement.resumeCommands = ''
      systemctl restart colorctl.service
    '';

    # Systemd oneshot service that executes on boot
    systemd.services.colorctl = {
      description = "Apply Colorful Motherboard Fan Curves and RGB Settings";
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart =
          let
            fanCmd =
              if cfg.fan.enable then
                if cfg.fan.customPoints != null then
                  "${cfg.package}/bin/colorctl fan set-curve --fan ${cfg.fan.header} --points ${cfg.fan.customPoints}"
                else
                  "${cfg.package}/bin/colorctl fan set-curve --fan ${cfg.fan.header} --profile ${cfg.fan.profile}"
              else
                ":";

            rgbCmd =
              if cfg.rgb.enable then
                "${cfg.package}/bin/colorctl rgb set --channel ${cfg.rgb.channel} --color '${cfg.rgb.color}' --brightness ${toString cfg.rgb.brightness}"
              else
                ":";
          in
          pkgs.writeShellScript "colorctl-apply" ''
            ${optionalString cfg.fan.enable "${fanCmd}"}
            ${optionalString cfg.rgb.enable "${rgbCmd}"}
          '';
      };
    };
  };
}
