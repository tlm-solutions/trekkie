{ config, inputs, ... }:
{
  TLMS.trekkie = {
    # Unlike the production, we do not reverse-proxy the trekkie, we just expose
    # port directly to the host vm.
    enable = true;
    host = "0.0.0.0";
    saltPath = "${inputs.self}/tests/vm/test-pw";
    port = 8060;
    database = {
      host = "127.0.0.1";
      port = config.services.postgresql.port;
      passwordFile = ./test-pw;
    };
    redis = {
      port = 6379;
      host = "localhost";
    };
    logLevel = "info";
  };
  systemd.services."trekkie" = {
    after = [ "postgresql.service" ];
    wants = [ "postgresql.service" ];
  };

  services = {
    redis.servers."trekkie" = {
      enable = true;
      bind = config.TLMS.trekkie.redis.host;
      port = config.TLMS.trekkie.redis.port;
    };
  };
}
