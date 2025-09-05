{
  description = "GitHub CLI extension to mark a PR as ready for review and enable auto-merge";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    git-hooks,
    nixpkgs,
  }: let
    forEachSystem = nixpkgs.lib.genAttrs [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-darwin"
      "x86_64-linux"
    ];
  in {
    checks = forEachSystem (system: let
      pre-commit-check = git-hooks.lib.${system}.run {
        src = ./.;
        hooks = {
          actionlint.enable = true;
          alejandra = {
            enable = true;
            settings = {
              verbosity = "quiet";
            };
          };
          cargo-check.enable = true;
          clippy.enable = true;
          deadnix.enable = true;
          prettier.enable = true;
          rustfmt.enable = true;
          shellcheck.enable = true;
          shfmt.enable = true;
          statix.enable = true;
        };
      };
    in {
      inherit pre-commit-check;
    });

    devShells = forEachSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      default = pkgs.mkShell {
        inherit (self.checks.${system}.pre-commit-check) shellHook;
        buildInputs = with pkgs; [
          rustc
          cargo
        ];
      };
    });
  };
}
