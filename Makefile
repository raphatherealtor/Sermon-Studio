# Sermon Studio — developer tasks.
.PHONY: help canon cli app test clean rebuild

help:
	@echo "Sermon Studio targets:"
	@echo "  make canon    Build canon.db from data/raw (via tools/etl.py)"
	@echo "  make cli      Build the headless CLI (target/release/sermon)"
	@echo "  make app      Build the desktop app (.AppImage + .deb)"
	@echo "  make test     Run the core test suite"
	@echo "  make rebuild  Rebuild pastor.db from ~/Sermons"
	@echo "  make clean    Remove build artifacts"

canon:
	scripts/build-canon.sh

cli:
	cargo build --release -p sermon

app:
	scripts/build-app.sh

test:
	cargo test -p sermon_core

rebuild:
	scripts/rebuild-index.sh

clean:
	cargo clean
	rm -rf ui/dist
