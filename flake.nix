{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay/master";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    cargo2nix = {
      url = "github:cargo2nix/cargo2nix/release-0.12";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };
  };

  outputs = {
    self,
    nixpkgs,
    cargo2nix,
    ...
  }: {
    overlays = {
      default = pkgs': pkgs: {
        quaigh = pkgs'.rustPkgs.workspace.quaigh {};
        rustPkgs = pkgs'.rustBuilder.makePackageSet {
          rustVersion = "1.86.0";
          # You can regenerate Cargo.nix using this command:
          #   nix run github:cargo2nix/cargo2nix/v0.12.0
          packageFun = import ./Cargo.nix;

          packageOverrides = pkgs:
            pkgs.rustBuilder.overrides.all
            ++ [
              (pkgs.rustBuilder.rustLib.makeOverride {
                name = "rustsat-kissat";
                overrideAttrs = {
                  buildInputs =
                    [
                      pkgs.kissat
                    ]
                    ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
                      pkgs.openssl
                    ];
                  patches = [
                    ./nix/patches/rustsat-kissat.patch
                  ];
                  NIX_KISSAT_DIR = "${pkgs.kissat.lib}";
                };
              })
            ];
        };
      };
    };

    legacyPackages = nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"] (
      system:
        import nixpkgs {
          inherit system;
          overlays = [cargo2nix.overlays.default self.overlays.default];
        }
    );
    packages = nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"] (system: {
      inherit (self.legacyPackages."${system}") quaigh;
      default = self.legacyPackages."${system}".quaigh;
    });
  };
}
