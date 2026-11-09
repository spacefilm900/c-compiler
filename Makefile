# Build the compiler the harness expects at build/compiler.
CARGO ?= cargo

.PHONY: all test wacc clean

all: build/compiler

build/compiler: Cargo.toml $(wildcard src/*.rs)
	$(CARGO) build --release
	mkdir -p build
	cp -f target/release/compiler $@

test: build/compiler
	$(CARGO) test
	python3 harness/run_tests.py

wacc: build/compiler
	scripts/fetch_wacc_tests.sh
	python3 harness/run_wacc.py

clean:
	$(CARGO) clean
	rm -f build/compiler
