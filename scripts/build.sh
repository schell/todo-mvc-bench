#! /bin/bash

if [ -z ${GITHUB_REF+x} ]; then
    export GITHUB_REF=`git rev-parse --symbolic-full-name HEAD`
fi

export PATH=$PATH:$HOME/.cargo/bin

if hash rustup 2>/dev/null; then
    echo "Have rustup, skipping installation..."
else
    echo "Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    echo "  done installing the rust toolchain."
fi

if hash wasm-pack 2>/dev/null; then
    echo "Have wasm-pack, skipping installation..."
else
    echo "Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
    echo "  done installing wasm-pack."
fi

echo "Building web-sys-examples w/ wasm-pack..."
mkdir -p release
wasm-pack build --release --target no-modules || exit 1
cp -R pkg index.html style.css release/
sleep 1
tar czvf release.tar.gz release || exit 1
sleep 1
ls -lah release.tar.gz
echo "Done building on ${GITHUB_REF}"
