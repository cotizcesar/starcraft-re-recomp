SC_MAP_DIR ?= /tmp/sc_maps
SC_INSTALL ?= /path/to/StarCraft
SC_CACHE ?= /tmp/galaxycraft-retail

.PHONY: test fmt clippy oracle orders regions sim battle match view view-retail shot ui-shots campaign audit triggers verify-runtime verify retail-verify retail-test retail-campaign retail-extract

# Everything that can run without an extracted retail install.
verify: test clippy orders oracle audit

triggers:
	python3 tools/trigger_ids.py $(SC_MAP_DIR)

test:
	cargo test

fmt:
	cargo fmt

clippy:
	cargo clippy --all-targets -- -D warnings

oracle:
	python3 tools/oracle.py fixtures

# Recovered order dispatch table (189 rows) and lane validation.
orders:
	cargo run --quiet --bin sc_orders

# Region graph for an extracted map (set SC_REGION_CHK/SC_REGION_VF4).
regions:
	cargo run --release --bin sc_regions -- $(SC_REGION_CHK) $(SC_REGION_VF4)

sim:
	cargo run --bin sc_sim

battle:
	cargo run --bin sc_sim -- battle 3 marine 1 zealot

match:
	cargo run --release --bin sc_match -- --ticks 20000 --verbose

# Playable graphical client and a headless frame render.
view:
	cargo run --release --bin sc_view

# Play using the player's own StarCraft installation (no bundled assets).
view-retail:
	cargo run --release --bin sc_view -- --install "$(SC_INSTALL)"

# Verify CASC + all 323 shipped MPQ maps against the live installation.
retail-verify:
	cargo run --release --bin sc_assets -- --install "$(SC_INSTALL)" verify

# Read/step every shipped map straight from Maps/*.scm + CASC.
retail-campaign:
	cargo run --release --bin sc_campaign -- --install "$(SC_INSTALL)" --ticks 1200

# Materialise only the derived subset the client needs into a local cache.
retail-extract:
	cargo run --release --bin sc_assets -- --install "$(SC_INSTALL)" extract --out "$(SC_CACHE)"

# Original-install integration tests (CASC, MPQ, hashes, render).
retail-test:
	SC_INSTALL="$(SC_INSTALL)" cargo test --test retail -- --nocapture

shot:
	cargo run --release --bin sc_shot -- --out /tmp/galaxycraft.bmp

# Headless screenshots of the retail UI surfaces (set SC_INSTALL).
ui-shots:
	for s in intro menu briefing mission selection pause leaderboard stats score; do \
		cargo run --release --quiet --bin sc_view -- --install "$(SC_INSTALL)" \
			--shot "/tmp/gc-$$s.bmp" --shot-screen "$$s"; \
	done

# Histogram of trigger/unit/tileset ids over the shipped map corpus (set SC_MAP_DIR).
audit:
	cargo run --quiet --release --bin sc_audit -- --maps $(or $(SC_MAP_DIR),/tmp/sc_maps)

# Every shipped map, extracted under /tmp/sc_maps with /tmp/mpq_tool.
campaign:
	cargo run --release --bin sc_campaign -- \
		--maps /tmp/sc_maps --tiles /tmp/tiles/TileSet --ticks 1200

# Rebuilt runtime image from the read-only snapshot of the running game.
verify-runtime:
	python3 tools/verify_runtime_tables.py /tmp/sc_mem/sc_dump.exe
