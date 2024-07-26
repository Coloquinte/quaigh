{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.05";
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix/release-0.11.0";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    cargo2nix,
    ...
  }: {
    packages = nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"] (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [cargo2nix.overlays.default];
      };
      rustPkgs = pkgs.rustBuilder.makePackageSet {
        rustVersion = "1.75.0";
        # You can regenerate Cargo.nix using this command:
        #   nix run github:cargo2nix/cargo2nix
        packageFun = import ./Cargo.nix;

        packageOverrides = pkgs:
          pkgs.rustBuilder.overrides.all
          ++ [
            (pkgs.rustBuilder.rustLib.makeOverride {
              name = "rustsat-kissat";
              overrideAttrs = {
                buildInputs = [
                  pkgs.kissat
                ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                  pkgs.openssl
                ];
                patches = [
                  ./nix/patches/rustsat-kissat.patch
                ];
                NIX_KISSAT_DIR="${pkgs.kissat.lib}";
              };
            })
          ];
      };
      self = {
        quaigh = rustPkgs.workspace.quaigh {};
        default = self.quaigh;
      };
    in
      self);
  };
}
