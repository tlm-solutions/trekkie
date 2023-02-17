{ pkgs, lib, config, inputs, ... }: {
  services.postgresql = {
    enable = true;
    port = 5432;
    package = pkgs.postgresql_14;
    ensureDatabases = [ "dvbdump" ];
    ensureUsers = [
      {
        name = "dvbdump";
        ensurePermissions = {
          "DATABASE dvbdump" = "ALL PRIVILEGES";
          "ALL TABLES IN SCHEMA public" = "ALL";
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

      # fix the permissions for dvbdump user on migration-created tables
      $PSQL -c "GRANT ALL ON DATABASE dvbdump TO dvbdump;"
      $PSQL -d dvbdump -c "GRANT ALL ON ALL TABLES IN SCHEMA public TO dvbdump;"
      $PSQL -d dvbdump -c "GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO dvbdump;"
      unset DATABASE_URL
    '';
  };

}
