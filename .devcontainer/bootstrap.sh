# Install Rustup Installer
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Install rust and other components
readonly RUST_UP_PATH=$HOME/.cargo/bin
$RUST_UP_PATH/rustup update stable && $RUST_UP_PATH/rustup default stable && $RUST_UP_PATH/rustup component add clippy rustfmt

# Install wasm32-wasi target
$RUST_UP_PATH/rustup target add wasm32-wasi wasm32-unknown-unknown

# Install k3d
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash