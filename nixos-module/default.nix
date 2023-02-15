{ pkgs, config, lib, ... }:
let
  cfg = config.TLMS.trekkie;
in
{
  options.TLMS.trekkie = with lib; {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = ''Wether to enable trekkie service'';
    };
    host = mkOption {
      type = types.str;
      default = "0.0.0.0";
      description = ''
        To which IP trekkie should bind.
      '';
    };
    port = mkOption {
      type = types.port;
      default = 8080;
      description = ''
        To which port should trekkie bind.
      '';
    };
    saltPath = mkOption {
      type = types.either types.path types.string;
      default = "/run/secrets/salt_path";
      description = ''
        File from which the password salt can be taken
      '';
    };
    database = {
      host = mkOption {
        type = types.str;
        default = "127.0.0.1";
        description = ''
          Database host
        '';
      };
      port = mkOption {
        type = types.port;
        default = 5354;
        description = ''
          Database port
        '';
      };
      passwordFile = mkOption {
        type = types.either types.path types.string;
        default = "";
        description = ''password file from which the postgres password can be read'';
      };
    };
    redis = {
      host = mkOption {
        type = types.str;
        default = "127.0.0.1";
        description = ''
          redis host
        '';
      };
      port = mkOption {
        type = types.port;
        default = 6379;
        description = ''
          redis port
        '';
      };
    };
    user = mkOption {
      type = types.str;
      default = "trekkie";
      description = ''systemd user'';
    };
    group = mkOption {
      type = types.str;
      default = "trekkie";
      description = ''group of systemd user'';
    };
    logLevel = mkOption {
      type = types.str;
      default = "info";
      description = ''log level of the application'';
    };
  };

  config = lib.mkIf cfg.enable {
    users.groups.TLMS-radio = {
      name = "TLMS-radio";
      members = [
        "wartrammer"
        "data-accumulator"
        "trekkie"
      ];
      gid = 1501;
    };

    systemd = {
      services = {
        "setup-trekkie" = {
          wantedBy = [ "multi-user.target" ];
          script = ''
            mkdir -p /var/lib/trekkie
            chmod 755 /var/lib/trekkie
            chown ${config.systemd.services.trekkie.serviceConfig.User} /var/lib/trekkie
            chgrp ${config.users.groups.TLMS-radio.name} /var/lib/trekkie
            mkdir -p /var/lib/trekkie/gpx
            chmod 755 /var/lib/trekkie/gpx
            chown ${config.systemd.services.trekkie.serviceConfig.User} /var/lib/trekkie/gpx
            chgrp ${config.users.groups.TLMS-radio.name} /var/lib/trekkie/gpx

          '';

          serviceConfig = {
            Type = "oneshot";
          };
        };

        "trekkie" = {
          enable = true;
          wantedBy = [ "multi-user.target" "setup-trekkie.service" ];

          script = ''
            exec ${pkgs.trekkie}/bin/trekkie --api-host ${cfg.host} --port ${toString cfg.port}&
          '';

          environment = {
            "POSTGRES_PASSWORD_PATH" = "${cfg.database.passwordFile}";
            "RUST_LOG" = "${cfg.logLevel}";
            "RUST_BACKTRACE" = if (cfg.logLevel == "info") then "0" else "1";
            "POSTGRES_HOST" = "${cfg.database.host}";
            "POSTGRES_PORT" = "${toString cfg.database.port}";
            "SALT_PATH" = "${cfg.saltPath}";
            "REDIS_PORT" = "${toString cfg.redis.port}";
            "REDIS_HOST" = "${cfg.redis.host}";
          };

          serviceConfig = {
            Type = "forking";
            User = cfg.user;
            Restart = "always";
          };
        };
      };
    };
    services.redis.servers."trekkie" = {
      enable = true;
      port = cfg.redis.port;
      bind = cfg.redis.host;
    };

    # user accounts for systemd units
    users.users."${cfg.user}" = {
      name = "${cfg.user}";
      description = "This guy runs trekkie";
      isNormalUser = false;
      isSystemUser = true;
      group = cfg.group;
      uid = 1502;
      extraGroups = [ config.users.groups."TLMS-radio".name ];
    };
  };
}
