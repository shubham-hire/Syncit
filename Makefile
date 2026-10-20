SHELL := /bin/sh

CARGO ?= cargo
FLUTTER ?= flutter

.PHONY: test test-core test-ui format-check lint build sim

test: test-core
	@if command -v $(FLUTTER) >/dev/null 2>&1; then \
		cd app && $(FLUTTER) test --no-pub; \
	else \
		echo "ERROR: Flutter is not installed; install the pinned SDK from .fvmrc or run 'make test-core'."; \
		exit 2; \
	fi

test-core:
	$(CARGO) test --workspace --locked

test-ui:
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for UI tests" && exit 1)
	cd app && $(FLUTTER) test --no-pub

format-check:
	$(CARGO) fmt --all -- --check
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for Dart formatting checks" && exit 1)
	cd app && $(FLUTTER) format --set-exit-if-changed lib test

lint:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for Dart analysis" && exit 1)
	cd app && $(FLUTTER) analyze

build:
	$(CARGO) build --workspace --locked
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for application builds" && exit 1)
	cd app && $(FLUTTER) build apk --debug

sim:
	$(CARGO) run -p audiosync-sim --locked
