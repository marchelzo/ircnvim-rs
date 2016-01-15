### ircnvim-rs

This is the server part of [ircnvim](https://github.com/marchelzo/ircnvim).

Installation instructions:

    1. Install nightly Rust
    2. Clone this repository, and run `make && sudo make install`
    3. Create at least one profile in the configuration file

```sh
curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=nightly && \
    git clone https://github.com/marchelzo/ircnvim-rs && \
    cd ircnvim-rs && \
    make && \
    sudo make install
```
