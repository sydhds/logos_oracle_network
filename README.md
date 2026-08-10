
# logos_oracle_network

## LEZ dev setup

* Base: Ubuntu 24.04 + rustup + docker

* to install: sudo apt install unzip python3.12-dev pkgconf libpcsclite-dev
* From tutorial.md in https://github.com/logos-co/spel/pull/138
  * RISC0 toolchain: https://dev.risczero.com/api/zkvm/install
  * Compile spel: `git clone https://github.com/logos-co/spel.git` && `cargo build -p spel-framework -p spel-framework-core -p spel-framework-macros -p spel-client-gen -p spel`
  * Compile logos execution zone: `git clone https://github.com/logos-blockchain/logos-execution-zone.git && cd logos-execution-zone && git checkout v0.2.0`
    * Find the logos execution zone version in spel/spel-framework/Cargo.toml
    * Compile: `cargo build --release --features standalone -p sequencer_service` && `cargo build --release -p wallet`
  * Add spel & logos exec zone bin into $PATH: `vim ~/.bashrc` && set to `export PATH="$PATH:/home/ubuntu/.risc0/bin:/home/ubuntu/logos-execution-zone/target/release/:/home/ubuntu/spel/target/debug`
  * Test the setup: `wallet --version`, `spel --version`

## Launch the logos execution zone

RUST_LOG=info cargo run --features standalone -p sequencer_service -- lez/sequencer/service/configs/debug/sequencer_config.json

Note: 
* reset sequencer: `rm -rf logos-execution-zone/rocksdb`

## Wallet

* Create a public account
  * `wallet account new public`
* List accounts
  * `wallet account list`

Note:
* delete wallet info: `rm -rf ~/.lee`

## Deploy contract & interact with

* make deploy
  * OR (manual way): `wallet deploy-program methods/guest/target/riscv32im-risc0-zkvm-elf/docker/my_counter.bin`
  * Sequencer logs: `Validated transaction with hash c8f138b1d34ba952978fcf86bcb53119c0c6f3ed10287bb36534de6d649f5aea, including it in block`
* init contract: 
  * `spel initialize --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`
* increment contract:
  * `spel increment --amount 5 --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`
* read counter:
  * `spel pda counter`
  * `spel inspect "G1gkRm62LdJ2XWpj5NBHeHgdNgjrzqQuPnW4CL8GqNjm" --type CounterState`






