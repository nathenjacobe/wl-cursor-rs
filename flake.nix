{
  description = "wl-find-cursor-rs";

  inputs = {
    rust.url = "github:nathenjacobe/rust-flake";
    nixpkgs.follows = "rust/nixpkgs";
  };

  outputs = { nixpkgs, rust, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
      };
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "wl-find-cursor";
        version = "0.1.0";
        src = ./.;
        cargoLock.lockFile = ./Cargo.lock;
        buildInputs = [ pkgs.wayland ];
      };

      devShells.${system}.default = pkgs.mkShell {
        packages = [
          rust.packages.${system}.rustToolchain
          pkgs.wayland
        ];
      };
    };
}