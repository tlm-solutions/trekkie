{ naersk, src, lib, pkg-config, cmake, protobuf, postgresql, zlib }:

naersk.buildPackage {
  pname = "trekkie";
  version = "0.1.0";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config cmake ];
  buildInputs = [ protobuf zlib postgresql ];

  meta = with lib; {
    description = "Simple rust server which collects gps tracks and measurement intervals";
    homepage = "https://github.com/dump-dvb/trekkie";
  };
}
