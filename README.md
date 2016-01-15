### ircnvim-rs

This is the server part of ircnvim.

Installation instructions:

    1. Install nightly Rust
    2. Clone this repository, and run `make && sudo make install`

```sh
curl -sSf https://static.rust-lang.org/rustup.sh | sh -s -- --channel=nightly && \
    git clone https://github.com/marchelzo/ircnvim-rs && \
    cd ircnvim-rs && \
    make && \
    sudo make install
```
