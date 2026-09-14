{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        colorctlPkg = pkgs.callPackage ./nix/package.nix { };
      in
      {
        packages.default = colorctlPkg;
        packages.colorctl = colorctlPkg;

        devShells.default = pkgs.mkShell {
          buildInputs = [
            (pkgs.rust-bin.stable.latest.default.override {
              extensions = [
                "rust-src"
                "rust-analyzer"
              ];
            })
          ];
        };

        checks.build = colorctlPkg;
      }
    )
    // {
      overlays.default = final: prev: {
        colorctl = self.packages.${final.stdenv.hostPlatform.system}.default;
      };

      nixosModules.default = import ./nix/module.nix;
      nixosModules.colorctl = self.nixosModules.default;
    };
}
