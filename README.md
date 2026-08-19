
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

## Build oracle register contract

* `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_register` then `make build`
* Faster dev build (still requires the CARGO_TARGET_DIR export):
  * `RISC0_USE_DOCKER=0 cargo build -j 8 --release`
* Generate idl
  * `spel generate-idl methods/guest/src/bin/oracle_register.rs > oracle_register-idl.json`

## Deploy it (WIP)

* Copy file
  * `cp -v /home/ubuntu/local_target/oracle_register/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin` 
  * dev build: `cp -v /home/ubuntu/local_target/oracle_register/riscv-guest/oracle_register-methods/oracle_register-guest/riscv32im-risc0-zkvm-elf/release/oracle_register.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin`
* `spel initialize --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`
* `spel register --owner 5EYkqoY3fXNGqUABDMaCFurivdofeaXUofpKnJ6NrQE3`

## Use oracle_register_client

* `cargo run -- /home/ubuntu/repos/logos_oracle_network/oracle_register/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_register.bin /home/ubuntu/lez-programs/programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`

## LEZ token program

* doc: `https://github.com/logos-blockchain/lez-programs/tree/main/docs/token`

* `git clone https://github.com/logos-blockchain/lez-programs.git`
  * build: `cargo risczero build --manifest-path ./programs/token/methods/guest/Cargo.toml`
    * copy .bin file: `cp -v /home/ubuntu/local_target/oracle_register_client/riscv32im-risc0-zkvm-elf/docker/token.bin programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`
      * Note: this is bc in this example the CARGO_TARGET_DIR was already set so you need to adapt here
   * deploy: `wallet deploy-program programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin`
   * generate idl: `spel generate-idl programs/token/methods/guest/src/bin/token.rs > artifacts/token-idl.json`
   * See avail cmd: `spel --idl artifacts/token-idl.json --help`
   * create a token: `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- new-fungible-definition --name "LON" --total-supply 21000 --definition-target-account daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj --holding-target-account 9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp --mint-authority daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj`
   * mint: `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- mint --amount-to-mint 100 --definition-account daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj --user-holding-account 9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp`
   * inspect:
     * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp" --type TokenHolding`
     * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj" --type TokenDefinition`
  * transfer:
    * Note: first is required a init account
    * `wallet account new public --label for_transfer_1`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- initialize-account --account-to-initialize 58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK --definition-account daZ1dGEHxU9UAYCK9QrfSPE6LutYP369A3DC8XnryQj`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin inspect "58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK" --type TokenHolding`
    * `spel --idl artifacts/token-idl.json -p programs/token/methods/guest/target/riscv32im-risc0-zkvm-elf/docker/token.bin -- transfer --sender 9DYb8L5nVTxYoYx7aKXQ1UU7J9fzY84LFzoAY4dQtghp --recipient 58mmyXYGG4btrmFD1BwoY94BPAQ7MJE3x1hWugSCbChK --amount-to-transfer 10`
    * 

## Resources

* lez-multisig: `https://github.com/logos-co/lez-multisig/blob/main/scripts/DEMO-RUNBOOK.md`

## oracle_prices contract

* Build
  * `export CARGO_TARGET_DIR=/home/ubuntu/local_target/oracle_prices`
  * `make build`
  * `cp -v ~/local_target/oracle_prices/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin`
  * `make idl`
* Build (no docker)
  * `RISC0_USE_DOCKER=0 cargo build -j 8 --release`
  * `cp -v ~/local_target/oracle_prices/riscv-guest/oracle_prices-methods/oracle_prices-guest/riscv32im-risc0-zkvm-elf/release/oracle_prices.bin  methods/guest/target/riscv32im-risc0-zkvm-elf/docker/oracle_prices.bin`
  * `make idl`
* Deploy
  * `make deploy`
* Init contract
  * `spel initialize`
* Init a feed
  * `spel initialize-feed --feed-id 0000000000000000000000000000000000000000000000000000000000000001`
* Publish a price
  * `spel publish-price --feed-id 0000000000000000000000000000000000000000000000000000000000000001 --price 1000 --decimals 8 --valid-count 3 --round 1000 --confidence 4242`
* Get price
  * `spel pda feed_price --feed-id 0000000000000000000000000000000000000000000000000000000000000001`
    * `spel inspect "NjvkDBbwv6dfxHfyGQR7seiGXQwcvDfkYPHmCmURHym" --type PriceState`
* Get feeds
  * `spel pda oracle_prices_account`
    * `spel inspect "5mprrVcUZgyMDRg4RD6pkwMHXK5DbwrPnEGZwm5ZKUHy" --type OraclePricesState` 



