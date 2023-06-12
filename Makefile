ENDPOINT ?= mainnet.eth.streamingfast.io:443
POSTGRESQL_DSN ?= psql://dev-node:insecure-change-me-in-prod@localhost:5432/dev-node?sslmode=disable&schema=substream1

.PHONY: build
build:
	cargo build --target wasm32-unknown-unknown --release
	./set_initial_block.sh

.PHONY: protogen
protogen:
	substreams protogen ./substreams.yaml --exclude-paths="sf/substreams,google"

.PHONY: clean
clean:
	rm -f schema_settings.sql src/settings/mod.rs
	cargo clean

.PHONY: package
package: build
	substreams pack substreams.yaml

.PHONY: sink_postgres
sink_postgres: build
	substreams-sink-postgres run '$(POSTGRESQL_DSN)' $(ENDPOINT) "./substreams.yaml" db_out

.PHONY: setup_postgres
setup_postgres: build
	cat schema.sql schema_settings.sql | substreams-sink-postgres setup '$(POSTGRESQL_DSN)' /dev/stdin
