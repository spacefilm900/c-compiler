# Build the compiler the harness expects at build/compiler.
CARGO ?= cargo

.PHONY: all test wacc csmith csmith-campaign otawk torture perf awk-extra doom lua sqlite clean

all: build/compiler

build/compiler: Cargo.toml $(wildcard src/*.rs src/*/*.rs)
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

# Local coverage campaign. Override SEED_START, COUNT, and OUT.
SEED_START ?= 1
COUNT ?= 100
OUT ?= csmith-campaign-results
csmith-campaign: build/compiler
	scripts/fetch_csmith.sh
	python3 tools/csmith_campaign.py --compiler build/compiler --seed-start $(SEED_START) --count $(COUNT) --opt O0 --opt O2 --out $(OUT)

otawk: build/compiler
	scripts/fetch_onetrueawk.sh
	python3 harness/run_onetrueawk.py

torture: build/compiler
	scripts/fetch_gcc_torture.sh
	python3 harness/run_gcc_torture.py

doom: build/compiler
	scripts/fetch_doomgeneric.sh
	scripts/fetch_freedoom.sh
	python3 harness/run_doom.py

lua: build/compiler
	scripts/fetch_lua.sh
	python3 harness/run_lua.py

sqlite: build/compiler
	scripts/fetch_sqlite.sh
	python3 harness/run_sqlite.py

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
