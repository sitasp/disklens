.PHONY: test clippy fmt bindings

test:
	cargo test --manifest-path core/Cargo.toml --workspace

clippy:
	cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings

fmt:
	cargo fmt --manifest-path core/Cargo.toml --all

bindings:
	./scripts/build-rust.sh
