{ buildPackage, gnumake, src, lib, pkg-config, cmake, protobuf, postgresql, zlib, openssl}:

buildPackage {
  pname = "trekkie";
  version = "0.2.1";

  src = ./.;

  cargoSha256 = lib.fakeSha256;

  nativeBuildInputs = [ pkg-config cmake gnumake ];
  buildInputs = [ protobuf zlib postgresql openssl ];

  meta = {
    description = "Simple rust server which collects gps tracks and measurement intervals";
    homepage = "https://github.com/tlm-solutions/trekkie";
  };
}
