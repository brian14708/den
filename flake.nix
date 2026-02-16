{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
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
      imports = [
        inputs.treefmt-nix.flakeModule
      ];
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
              hash = "sha256-a0bcVsFULdJHDK2Tl8At808INdQEb4x7v3ATzLlc6pc=";
            };

            env.NEXT_TELEMETRY_DISABLED = "1";

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
                ];
              };

          treefmt = {
            projectRootFile = "flake.nix";
            programs = {
              nixfmt.enable = true;
              rustfmt = {
                enable = true;
                package = rustToolchain pkgs;
              };
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
                commonArgs = {
                  inherit pname;
                  src = craneLib.cleanCargoSource ./.;
                  strictDeps = true;
                };
              in
              craneLib.buildPackage (
                commonArgs
                // {
                  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
                  preBuild = ''
                    mkdir -p web/out
                    cp -r ${frontend}/* web/out/
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
