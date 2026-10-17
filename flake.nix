{
  description = "Todo ledger development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          # winit and wgpu load these at runtime with dlopen, so they must be on
          # the library path for the native GUI to open a window on Linux.
          guiRuntimeLibs = pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux (with pkgs; [
            wayland
            libxkbcommon
            libGL
            vulkan-loader
            libx11
            libxcursor
            libxrandr
            libxi
          ]);
        in f { inherit pkgs guiRuntimeLibs; });
    in {
      packages = forAllSystems ({ pkgs, guiRuntimeLibs }: rec {
        todo = pkgs.rustPlatform.buildRustPackage {
          pname = "todo";
          version = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;
          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./crates
              # Embedded into the binaries with include_bytes!/include_str!.
              ./docs/protocols
              ./docs/themes
            ];
          };
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "todo-cli" "-p" "todo-gui" ];
          nativeBuildInputs = [ pkgs.pkg-config pkgs.cmake pkgs.rustPlatform.bindgenHook ];
          # Tests run through `./dev test`; this output only builds the binaries.
          doCheck = false;
          dontUseCmakeConfigure = true;
          postFixup = pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isLinux ''
            patchelf --add-rpath ${pkgs.lib.makeLibraryPath guiRuntimeLibs} $out/bin/todo-gui
          '';
        };
        default = todo;
      });

      devShells = forAllSystems ({ pkgs, guiRuntimeLibs }: {
        default = pkgs.mkShell {
          packages = with pkgs; [ cargo rustc rustfmt clippy pkg-config cmake clang git ];
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath guiRuntimeLibs;
        };
      });
    };
}
