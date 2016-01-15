all: ircnvim

ircnvim:
	cargo build --release

install:
	cp ./target/release/ircnvim /usr/local/bin/ircnvim
