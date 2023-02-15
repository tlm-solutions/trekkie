# add-hoc config for a test VM. 
{ inputs, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/virtualisation/qemu-vm.nix"
    ./trekkie.nix
    ./postgres.nix
  ];

  fileSystems = {
    "/" = {
      device = "/dev/disk/by-label/nixos";
      fsType = "ext4";
    };
  };

  boot = {
    kernelParams = [ "console=ttyS0" "boot.shell_on_fail" ];
    loader.timeout = 5;
  };

  virtualisation = {
    diskSize = 512;
    memorySize = 512;
  };

  networking.firewall.enable = false;

  users.mutableUsers = false;
  users.users.root.password = "lol";
  services.openssh = {
    enable = true;
    permitRootLogin = "yes";
  };

  system.stateVersion = "22.11";
}
