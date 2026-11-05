# Tree Canopy Density (Rust Mock Plugin)

This plugin demonstrates a high-performance **native compiled binary** analytical plugin for the **Aerial Analytics Platform**.

It implements the exact same 4-file domain contract and CLI interface (`--healthcheck`, `--input`, `--output`) as Python plugins, but compiles directly to native machine code for maximum throughput.

---

## Plugin Specification

- **Plugin ID:** `tree-canopy-density`
- **Runtime:** `binary`
- **Entrypoint:** `bin/mock_rust`
- **Category:** `tree_detection`

---

## Building the Binary

### Linux / macOS

```bash
# From workspace root:
cargo build --release --manifest-path plugins/mock_rust/Cargo.toml
mkdir -p plugins/mock_rust/bin
cp plugins/mock_rust/target/release/mock_rust plugins/mock_rust/bin/
```

### Windows (PowerShell)

```powershell
cargo build --release --manifest-path plugins\mock_rust\Cargo.toml
if (!(Test-Path plugins\mock_rust\bin)) { New-Item -ItemType Directory -Path plugins\mock_rust\bin }
Copy-Item plugins\mock_rust\target\release\mock_rust.exe plugins\mock_rust\bin\
```

---

## Testing & Validation

### Healthcheck

```bash
./plugins/mock_rust/bin/mock_rust --healthcheck
# Output: {"status":"healthy","plugin_id":"tree-canopy-density","version":"1.0.0","runtime":"binary","compiler":"rustc"}
```

### Execution Run

```bash
./plugins/mock_rust/bin/mock_rust \
  --input plugins/mock_rust/execution_payload.yaml \
  --output /tmp/rust_test_result.json
```

### Schema Validation

```bash
check-jsonschema --schemafile schemas/plugin.schema.yaml plugins/mock_rust/manifest.yaml
check-jsonschema --schemafile schemas/parameters.schema.yaml plugins/mock_rust/parameters.yaml
check-jsonschema --schemafile schemas/inputs.schema.yaml plugins/mock_rust/inputs.yaml
check-jsonschema --schemafile schemas/outputs.schema.yaml plugins/mock_rust/outputs.yaml
check-jsonschema --schemafile schemas/execution_payload.schema.yaml plugins/mock_rust/execution_payload.yaml
check-jsonschema --schemafile schemas/execution_result.schema.yaml plugins/mock_rust/execution_result.yaml
```

---

## Specifications & Documentation

For the full field-by-field schema reference of all contracts and test fixtures (`execution_payload.yaml`, `execution_result.yaml`, `manifest.yaml`, etc.), refer to:

- [`plugins/template/README.md`](../template/README.md#field-reference-contracts--execution-fixtures)
- [`schemas/`](../../schemas/)
