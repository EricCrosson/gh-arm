{
  description = "GitHub CLI extension to mark a PR as ready for review and enable auto-merge";

  nixConfig.extra-substituters = ["https://ericcrosson-gh-arm.cachix.org"];
  nixConfig.extra-trusted-public-keys = ["ericcrosson-gh-arm.cachix.org-1:F/7TLvBGov4xjEE3MSRgNNmYRxPDxW8BUeAyy0rbVu="];

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
  };

  outputs = {
    # deadnix: skip
    self,
    nixpkgs,
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];
  in {
    packages = forEachSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      gh-arm = pkgs.rustPlatform.buildRustPackage {
        inherit (cargoToml.package) version;
        pname = cargoToml.package.name;
        src = ./.;
        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {};
        };
      };
    in {
      inherit gh-arm;
      default = gh-arm;
    });
  };
}
