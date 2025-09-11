all:
	cargo xtask build

install:
	install -D -m 0755 target/release/bcvk $(DESTDIR)/usr/bin/bcvk

makesudoinstall:
	make
	sudo make install

# No additional configuration needed anymore
