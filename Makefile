install:
	c b --release; \
	cp ./target/release/notebk /usr/local/bin/notebk

backup:
	cp -r ~/personal/notebook ~/personal/bk
