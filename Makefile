INSTALL_PATH ?= $(HOME)/.local/bin/flockfly

.PHONY: build install clean test integration-test

build:
	cargo build --release

install: build
	mkdir -p $(dir $(INSTALL_PATH))
	cp target/release/flockfly $(INSTALL_PATH)
	@echo "Installed to $(INSTALL_PATH)"

clean:
	cargo clean

test:
	cargo test

integration-test:
	docker build -t flockfly-integration -f tests/integration/Dockerfile .
	docker run --rm flockfly-integration
