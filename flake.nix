{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      flake-parts,
      crane,
      nixpkgs,
      rust-overlay,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      perSystem =
        {
          self',
          system,
          pkgs,
          ...
        }:
        let
          rustToolchain = p: p.rust-bin.stable.latest.default;
          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
          pname = "den";

          frontend = pkgs.stdenv.mkDerivation (finalAttrs: {
            pname = "den-frontend";
            version = "0.1.0";
            src = ./web;

            nativeBuildInputs = [
              pkgs.nodejs
              pkgs.pnpmConfigHook
              pkgs.pnpm_10
            ];

            pnpmDeps = pkgs.fetchPnpmDeps {
              inherit (finalAttrs) pname version src;
              pnpm = pkgs.pnpm_10;
              fetcherVersion = 3;
              hash = "sha256-ibPkx7q3BypBN35Grtn1H/Oo9OfgGvXGTaDInp648uE=";
            };

            buildPhase = ''
              runHook preBuild
              pnpm run build
              runHook postBuild
            '';

            installPhase = ''
              runHook preInstall
              cp -r out $out
              runHook postInstall
            '';
          });
        in
        {
          _module.args.pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          devShells.default =
            (craneLib.devShell.override {
              mkShell = pkgs.mkShell.override {
                stdenv =
                  if pkgs.stdenv.hostPlatform.isDarwin then
                    pkgs.clangStdenv
                  else
                    pkgs.stdenvAdapters.useMoldLinker pkgs.clangStdenv;
              };
            })
              {
                packages = [
                  pkgs.nodejs
                  pkgs.pnpm_10
                  pkgs.sqlx-cli
                  pkgs.openssl
                  pkgs.pkg-config
                ];
                env = {
                  OPENSSL_DIR = "${pkgs.openssl.dev}";
                  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
                };
              };

          apps = {
            default = {
              type = "app";
              program = "${self'.packages.default}/bin/${pname}";
              meta.description = "Personal agent hub & dashboard";
            };
          };

          packages = {
            inherit frontend;
            default =
              let
                src = pkgs.lib.cleanSourceWith {
                  src = ./.;
                  filter =
                    path: type:
                    (craneLib.filterCargoSources path type)
                    || (type == "directory" && baseNameOf path == "migrations")
                    || (builtins.match ".*\\.sql$" path != null);
                };
                commonArgs = {
                  inherit pname src;
                  strictDeps = true;
                  nativeBuildInputs = [ pkgs.pkg-config ];
                  buildInputs = [ pkgs.openssl ];
                };
              in
              craneLib.buildPackage (
                commonArgs
                // {
                  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
                  postInstall = ''
                    mkdir -p $out/share/${pname}/web/out
                    cp -r ${frontend}/* $out/share/${pname}/web/out/
                    chmod -R u+w $out/share/${pname}/web/out
                  '';
                }
              );
            oci = pkgs.dockerTools.buildImage {
              name = pname;
              tag = "latest";
              copyToRoot = [ self'.packages.default ];
              config = {
                Cmd = [ "/bin/${pname}" ];
              };
            };
          };
          checks = {
          }
          // (pkgs.lib.mapAttrs' (n: pkgs.lib.nameValuePair "package-${n}") self'.packages);
        };
      flake = {
      };
    };
}
