{
  description = "kontena — container runtime management daemon";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }: let
    forAllSystems = nixpkgs.lib.genAttrs [
      "aarch64-darwin"
      "x86_64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ];
  in {
    packages = forAllSystems (system: let
      pkgs = import nixpkgs { inherit system; };
    in {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "kontena";
        version = "0.1.0";
        src = self;
        cargoHash = "sha256-zoBDD0pe3Zm85mU8HSUO0wAtuSFCr1+wf0KEeXvkmgk=";
      };
    });

    overlays.default = final: prev: {
      kontena = self.packages.${final.system}.default;
    };
  };
}
