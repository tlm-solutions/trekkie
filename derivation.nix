{ naersk, gnumake, src, lib, pkg-config, cmake, protobuf, postgresql, zlib, openssl}:

naersk.buildPackage {
  pname = "trekkie";
  version = "0.1.0";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config cmake gnumake ];
  buildInputs = [ protobuf zlib postgresql openssl ];

  meta = {
    description = "Simple rust server which collects gps tracks and measurement intervals";
    homepage = "https://github.com/dump-dvb/trekkie";
  };
}
