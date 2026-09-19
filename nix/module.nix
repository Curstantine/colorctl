{
  config,
  lib,
  pkgs,
  ...
}:

with lib;

let
  cfg = config.services.colorctl;
  enumOrListOf =
    enums: types.coercedTo (types.enum enums) (x: [ x ]) (types.listOf (types.enum enums));

  fanHeaderType = enumOrListOf [
    "all"
    "cpu"
    "cha_fan1"
    "cha_fan2"
    "cha_fan3"
    "pump"
  ];

  rgbChannelType = enumOrListOf [
    "all"
    "led"
    "12v_1"
    "12v_2"
    "5v_1"
    "5v_2"
    "5v_3"
  ];

  fanSubmodule =
    { ... }:
    {
      options = {
        enable = mkOption {
          type = types.bool;
          default = true;
          description = "Whether to apply this fan curve configuration.";
        };

        headers = mkOption {
          type = fanHeaderType;
          default = [ "all" ];
          example = [
            "all"
            "cpu"
          ];
          description = "Target fan header(s) for this curve ('all', 'cpu', 'cha_fan1', 'cha_fan2', 'cha_fan3', 'pump').";
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
      };
    };

  rgbSubmodule =
    { ... }:
    {
      options = {
        enable = mkOption {
          type = types.bool;
          default = true;
          description = "Whether to apply this RGB configuration.";
        };

        mode = mkOption {
          type = types.enum [
            "set"
            "off"
          ];
          default = "set";
          description = "RGB action mode: 'set' to configure color/brightness, or 'off' to turn off lighting.";
        };

        channels = mkOption {
          type = rgbChannelType;
          default = [ "all" ];
          example = [
            "12v_1"
            "5v_1"
          ];
          description = "Target RGB channel(s) ('all', 'led', '12v_1', '12v_2', '5v_1', '5v_2', '5v_3').";
        };

        color = mkOption {
          type = types.str;
          default = "cyan";
          example = "#ff0088";
          description = "Color to set (color name or hex). Ignored when mode is 'off'.";
        };

        brightness = mkOption {
          type = types.ints.between 0 100;
          default = 100;
          description = "RGB brightness level (0-100%).";
        };
      };
    };

  # coercedTo can't be used here because its "from" type can't itself be a
  # submodule (submodules carry merge semantics coercedTo can't reconcile).
  submoduleOrListOf =
    submodule:
    let
      elemType = types.submodule submodule;
    in
    types.either elemType (types.listOf elemType);

  toList = x: if builtins.isList x then x else [ x ];

  fanConfigType = submoduleOrListOf fanSubmodule;
  rgbConfigType = submoduleOrListOf rgbSubmodule;
in
{
  options.services.colorctl = {
    enable = mkEnableOption "Colorful motherboard RGB lighting and fan curve management";

    package = mkOption {
      type = types.package;
      default = pkgs.colorctl;
      description = "The colorctl package to use.";
    };

    fan = mkOption {
      type = fanConfigType;
      default = {
        headers = [ "all" ];
        profile = "quiet";
      };
      description = ''
        Fan curve configuration. Either a single attrset (applies to the
        given headers) or a list of attrsets targeting different headers.
      '';
      example = literalExpression ''
        # flat form
        {
          headers = [ "all" "cpu" ];
          profile = "quiet";
        }

        # list form
        [
          {
            headers = [ "all" "cpu" ];
            profile = "quiet";
          }
          {
            headers = [ "pump" ];
            profile = "full";
          }
        ]
      '';
    };

    rgb = mkOption {
      type = rgbConfigType;
      default = {
        channels = [ "all" ];
        mode = "off";
      };
      description = ''
        RGB lighting configuration. Either a single attrset (applies to the
        given channels) or a list of attrsets targeting different channels.
      '';
      example = literalExpression ''
        # flat form
        {
          channels = [ "led" ];
          color = "#ff0088";
          brightness = 80;
        }

        # list form
        [
          {
            channels = [ "12v_1" "5v_1" ];
            mode = "off";
          }
          {
            channels = [ "led" ];
            color = "#ff0088";
            brightness = 80;
          }
        ]
      '';
    };
  };

  config = mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    # Udev rule allowing unprivileged users to control RGB via /dev/hidraw
    services.udev.extraRules = ''
      KERNEL=="hidraw*", ATTRS{idVendor}=="2f4c", ATTRS{idProduct}=="1000", MODE="0666", TAG+="uaccess"
    '';

    # Ensure shared state directory exists with write permissions for colorctl
    systemd.tmpfiles.rules = [
      "d /var/lib/colorctl 0777 root root -"
    ];

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
            fanList = toList cfg.fan;
            rgbList = toList cfg.rgb;

            fanCmds = concatMap (
              f:
              optionals f.enable (
                map (
                  h:
                  if f.customPoints != null then
                    "${cfg.package}/bin/colorctl fan set-curve --fan '${h}' --points '${f.customPoints}'"
                  else
                    "${cfg.package}/bin/colorctl fan set-curve --fan '${h}' --profile '${f.profile}'"
                ) f.headers
              )
            ) fanList;

            rgbCmds = concatMap (
              r:
              optionals (r.enable && r.channels != [ ]) [
                (
                  let
                    channelsArg = concatStringsSep "," r.channels;
                  in
                  if r.mode == "off" then
                    "${cfg.package}/bin/colorctl rgb off --channel '${channelsArg}'"
                  else
                    "${cfg.package}/bin/colorctl rgb set --channel '${channelsArg}' --color '${r.color}' --brightness ${toString r.brightness}"
                )
              ]
            ) rgbList;
          in
          pkgs.writeShellScript "colorctl-apply" (concatStringsSep "\n" (fanCmds ++ rgbCmds));
      };
    };
  };
}
