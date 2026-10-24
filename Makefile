SHELL := /bin/sh

CARGO ?= cargo
FLUTTER ?= flutter

.PHONY: test test-core test-ui test-landing format-check lint lint-landing build build-landing sim host listener

test: test-core
	@if command -v $(FLUTTER) >/dev/null 2>&1; then \
		cd frontend/flutter-app && $(FLUTTER) test --no-pub; \
	else \
		echo "ERROR: Flutter is not installed; install the pinned SDK from .fvmrc or run 'make test-core'."; \
		exit 2; \
	fi

test-core:
	$(CARGO) test --workspace --locked

test-ui:
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for UI tests" && exit 1)
	cd frontend/flutter-app && $(FLUTTER) test --no-pub

test-landing:
	cd frontend/landing-page && npm run test

format-check:
	$(CARGO) fmt --all -- --check
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for Dart formatting checks" && exit 1)
	cd frontend/flutter-app && $(FLUTTER) format --set-exit-if-changed lib test

lint:
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for Dart analysis" && exit 1)
	cd frontend/flutter-app && $(FLUTTER) analyze

lint-landing:
	cd frontend/landing-page && npm run lint

build:
	$(CARGO) build --workspace --locked
	command -v $(FLUTTER) >/dev/null 2>&1 || (echo "Flutter is required for application builds" && exit 1)
	cd frontend/flutter-app && $(FLUTTER) build apk --debug

build-landing:
	cd frontend/landing-page && npm run build

sim:
	$(CARGO) run -p audiosync-sim --locked

host:
	$(CARGO) run -p audiosync-apps --bin audiosync-host --locked -- $(ARGS)

listener:
	$(CARGO) run -p audiosync-apps --bin audiosync-listener --locked -- $(ARGS)
