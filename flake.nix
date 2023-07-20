{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-23.05";

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    # needed to set up the schema in test vm database
    tlms-rs = {
      url = "github:tlm-solutions/tlms.rs";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs = inputs@{ self, nixpkgs, naersk, tlms-rs, utils, ... }:
    utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};

          package = pkgs.callPackage ./derivation.nix {
            naersk = naersk.lib.${system};
          };

          test-vm-pkg = self.nixosConfigurations.trekkie-mctest.config.system.build.vm;

        in
        rec {
          checks = packages;
          packages = {
            trekkie = package;
            test-vm = test-vm-pkg;
            test-vm-wrapper = pkgs.writeScript "trekkie-test-vm-wrapper"
            ''
              set -e

              echo Trekkie-McTest: enterprise-grade, free-range, grass fed testing vm
              echo
              echo "ALL RELEVANT SERVICES WILL BE EXPOSED TO THE HOST:"
              echo -e "Service\t\tPort"
              echo -e "SSH:\t\t2222\troot:lol"
              echo -e "postgres:\t8888"
              echo -e "trekkie:\t8060"
              echo -e "redis:\t\t8061"
              echo

              set -x
              export QEMU_NET_OPTS="hostfwd=tcp::2222-:22,hostfwd=tcp::8888-:5432,hostfwd=tcp::8060-:8060,hostfwd=tcp::8061-:6379"

              echo "running the vm now..."
              ${self.packages.${system}.test-vm}/bin/run-nixos-vm
            '';
            default = package;
          };

          # to get yourself a virtualized testing playground:
          # nix run .\#mctest
          apps = {
            mctest = {
              type = "app";
              program = "${self.packages.${system}.test-vm-wrapper}";
            };
          };
          devShells.default = pkgs.mkShell {
            nativeBuildInputs = (with packages.default; nativeBuildInputs ++ buildInputs) ++ [
              # python for running test scripts
              (pkgs.python3.withPackages (p: with p; [
                requests
              ]))
            ];
          };
        }
      ) // {
      overlays.default = final: prev: {
        inherit (self.packages.${prev.system})
          trekkie;
      };

      nixosModules = rec {
        default = trekkie;
        trekkie = import ./nixos-module;
      };

      # qemu vm for testing
      nixosConfigurations.trekkie-mctest = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit inputs; };
        modules = [
          self.nixosModules.default
          ./tests/vm

          {
            nixpkgs.overlays = [
              self.overlays.default
            ];
        }
        ];
      };
      hydraJobs =
        let
          hydraSystems = [
            "x86_64-linux"
            "aarch64-linux"
          ];
        in
        builtins.foldl'
          (hydraJobs: system:
            builtins.foldl'
              (hydraJobs: pkgName:
                nixpkgs.lib.recursiveUpdate hydraJobs {
                  ${pkgName}.${system} = self.packages.${system}.${pkgName};
                }
              )
              hydraJobs
              (builtins.attrNames self.packages.${system})
          )
          { }
          hydraSystems;
    };
}
