# Build the compiler the harness expects at build/compiler.
CARGO ?= cargo

.PHONY: all test wacc csmith otawk torture perf awk-extra clean

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

csmith: build/compiler
	scripts/fetch_csmith.sh
	python3 harness/run_csmith.py

otawk: build/compiler
	scripts/fetch_onetrueawk.sh
	python3 harness/run_onetrueawk.py

torture: build/compiler
	scripts/fetch_gcc_torture.sh
	python3 harness/run_gcc_torture.py

perf: build/compiler
	python3 harness/perf.py

awk-extra: build/compiler
	scripts/fetch_onetrueawk.sh
	scripts/build_onetrueawk.sh clang build/otawk-clang
	scripts/build_onetrueawk.sh ccomp build/otawk-ccomp
	AWK_CLANG=$(CURDIR)/build/otawk-clang/awk AWK_CCOMP=$(CURDIR)/build/otawk-ccomp/awk python3 harness/run_awk_extra.py

clean:
	$(CARGO) clean
	rm -f build/compiler
