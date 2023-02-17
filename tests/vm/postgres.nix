{ pkgs, lib, config, inputs, ... }: {
  services.postgresql = {
    enable = true;
    port = 5432;
    package = pkgs.postgresql_14;
    ensureDatabases = [ "dvbdump" ];
    ensureUsers = [
      {
        name = "grafana";
      }
      {
        name = "dvbdump";
        ensurePermissions = {
          "DATABASE dvbdump" = "ALL PRIVILEGES";
          "ALL TABLES IN SCHEMA public" = "ALL PRIVILEGES";
        };
      }
    ];
  };

  environment.systemPackages = [ inputs.tlms-rs.packages.x86_64-linux.run-migration ];

  systemd.services.postgresql = {
    unitConfig = {
      TimeoutStartSec=3000;
    };
    serviceConfig = {
      TimeoutSec = lib.mkForce 3000;
    };
    postStart = lib.mkAfter ''
      $PSQL -c "ALTER ROLE dvbdump WITH PASSWORD '$(cat ${inputs.self}/tests/vm/test-pw)';"

      export DATABASE_URL=postgres:///dvbdump
      ${inputs.tlms-rs.packages.x86_64-linux.run-migration}/bin/run-migration
      unset DATABASE_URL
    '';
  };

}
