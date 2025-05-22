{ lib, rustPlatform, pkg-config, cmake, openssl, libpq, ... }:

let
  manifest = (lib.importTOML ./Cargo.toml).package;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = manifest.name;
  inherit (manifest) version;

  src = lib.cleanSource ./.;

  cargoHash = "sha256-4Ev9PsJlgmBmptrLmpLfkHqA5IZEMJQz8lMoJu6F/UQ=";

  cargoBuildFlags = "-p ${finalAttrs.pname}";
  cargoTestFlags = "-p ${finalAttrs.pname}";

  nativeBuildInputs = [ pkg-config cmake ];

  buildInputs = [ openssl libpq ];

  meta = {
    mainProgram = "trekkie";
    description = "Simple rust server which collects gps tracks and measurement intervals";
    homepage = "https://github.com/tlm-solutions/trekkie";
  };
})

