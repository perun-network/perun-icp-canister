<h1 align="center">
    <a href="https://perun.network/"><img src=".asset/go_perun.png" alt="Perun" width="30%"></a>
</h1>

# Perun ICP Canister

This repository contains the Internet Computer canister for the [Perun state channel framework](https://perun.network) developed by [PolyCrypt](https://polycry.pt).
The canister is developed as part of the Dfinity grants program.
It contains the on-chain logic needed to operate payment channels on the internet computer.
In the future, it will connect to the [go-perun](https://github.com/hyperledger-labs/go-perun) client library so that applications can be written for it.

## :warning: Current problems

As, as of the time of this grant, to the best of our knowledge, [the ICP ledger canister does not allow arbitrary canisters to send ICP or listen for received ICP](https://forum.dfinity.org/t/integrating-with-the-internet-computer-ledger/2542) (expected to be available [after this grant](https://forum.dfinity.org/t/enable-canisters-to-hold-icp/6153/113)), and there are [no established fungible token standards](https://forum.dfinity.org/t/receiving-icp-in-canister/5329/6) yet (some promising ones are [already in development](https://forum.dfinity.org/t/thoughts-on-the-token-standard/4694/94), though), we have mocked all currency interactions:

* `deposit()` just takes the deposited amount as a number argument, currently trusting the value to be correct.
* `withdraw()` returns the withdrawn amount as a number, instead of actually sending any currency anywhere.

In the future, we plan to add the functionality to support real currency interactions, but this is most likely out of scope for the current grant's time limit.

Additionally, while the client-side logic for our channel framework is implemented, the IC adapter for the `go-perun` library is not part of this grant's scope, and we plan to connect our client library to the IC in a future follow-up grant.

## Protocol

A channel is opened by depositing funds for it into the contract by calling *Deposit*.
The participants of the channel can then do as many off-chain channel updates as they want.
When all participants come to the conclusion that the channel should be closed, they set the final flag on the channel state, and call *Conclude*.
All of them can then withdraw the outcome by calling *Withdraw*. 

*Dispute* is only needed if the participants do not arrive at a final channel state off-chain.
It allows any participant to enforce the last valid state, i.e., the mutually-signed state with the highest version number.
A dispute is initiated by calling *Dispute* with the latest available state.
A registered state can be refuted within a specified challenge period by calling *Dispute* with a newer state.
After the challenge period, the dispute can be concluded by calling *Conclude* and the funds can be withdrawn.

![state diagram](.asset/protocol.png)

## Test & Compile

```sh
cargo test --tests
./build.sh
```

## Example Walkthrough

We provide an example to show how to use the [ic-agent] crate to deposit funds
into the *Perun* canister. You will need Rust `1.56` or later.

1. Start a replica locally and deploy the *Perun* canister to it:  
```bash
dfx start --clean
dfx deploy # In a new terminal
```

2. Copy the *principal ID* from the terminal which looks like this: `rrkah-fqaaa-aaaaa-aaaaq-cai`.  
Make sure to copy the *Perun* canister ID, **not** the UI canister ID.

3. [Issue #4881 of cargo] needs to be worked around here since the example
needs to link against the canister as native lib.  
Change the `"cdylib"` in the [Cargo.toml] to `"lib"`.

4. Run the command below with the *Perun* canister ID that you copied:
```sh
RUST_LOG=info cargo run --example happy_walkthrough "rrkah-fqaaa-aaaaa-aaaaq-cai"
```
The output should look like this minus the comments:  
```sh
INFO  happy_walkthrough > URL: http://localhost:8000/
INFO  happy_walkthrough > Canister ID: rrkah-fqaaa-aaaaa-aaaaq-cai
# Bob and Alice start with 0 ICP in the channel.
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 0, now: 0 ICP
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 1, now: 0 ICP
# Bob and Alice deposit 242 and 194 ICP in the channel.
INFO  happy_walkthrough > Depositing       channel: 0x920c7366… for peer IDx: 0, add: 242 ICP
INFO  happy_walkthrough > Depositing       channel: 0x920c7366… for peer IDx: 1, add: 194 ICP
# Bob and Alice now have 242 and 194 ICP in the channel.
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 0, now: 242 ICP
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 1, now: 194 ICP
# They exchange a state…
# Bob and Alice can conclude the channel with a final state.
INFO  happy_walkthrough > Concluding       channel: 0x920c7366…
# Bob and Alice withdraw their funds from the channel.
INFO  happy_walkthrough > Withdrawing      channel: 0x920c7366… for peer IDx: 0
INFO  happy_walkthrough > Withdrawing      channel: 0x920c7366… for peer IDx: 1
# Bob and Alice now have 0 ICP in the channel.
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 0, now: 0 ICP
INFO  happy_walkthrough > Querying deposit channel: 0x920c7366… for peer IDx: 1, now: 0 ICP
```

[ic-agent]: https://crates.io/crates/ic-agent
[Cargo.toml]: Cargo.toml
[Issue #4881 of cargo]: https://github.com/rust-lang/cargo/issues/4881
