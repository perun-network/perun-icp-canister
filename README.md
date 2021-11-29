# icp_perun

Welcome to your new icp_perun project and to the internet computer development community. By default, creating a new project adds this README and some template files to your project directory. You can edit these template files to customize your project and to include your own code to speed up the development cycle.

To get started, you might want to explore the project directory structure and the default configuration file. Working with this project in your development environment will not affect any production deployment or identity tokens.

To learn more before you start working with icp_perun, see the following documentation available online:

- [Quick Start](https://sdk.dfinity.org/docs/quickstart/quickstart-intro.html)
- [SDK Developer Tools](https://sdk.dfinity.org/docs/developers-guide/sdk-guide.html)
- [Motoko Programming Language Guide](https://sdk.dfinity.org/docs/language-guide/motoko.html)
- [Motoko Language Quick Reference](https://sdk.dfinity.org/docs/language-guide/language-manual.html)
- [JavaScript API Reference](https://erxue-5aaaa-aaaab-qaagq-cai.raw.ic0.app)

# Test & Compile

```sh
cargo test --tests
./build.sh
```

# Example Walkthrough

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
