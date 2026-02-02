#!/bin/bash
# Generate sample .sync files for testing

SAMPLES_DIR="/Users/egamikohsuke/ekoh/projects/ato/capsuled-dev/apps/sync-rs/web-viewer/public/samples"
TEMP_DIR=$(mktemp -d)

# Minimal valid WASM module (empty module)
# This is the smallest valid WASM: magic number + version + empty sections
WASM_HEX="0061736d01000000"

create_wasm() {
    echo "$WASM_HEX" | xxd -r -p > "$1"
}

# ============================================
# Sample 1: Hello World Text
# ============================================
echo "Creating hello.sync..."
SAMPLE_DIR="$TEMP_DIR/hello"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "text/plain"
display_ext = "txt"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 86400
timeout = 30

[permissions]
allow_hosts = []
allow_env = []

[ownership]
write_allowed = false

[verification]
enabled = false
EOF

echo "Hello, World!

This is a sample .sync file containing plain text.
You can view this content in the sync-rs Web Viewer.

Features demonstrated:
- Plain text payload
- Basic manifest structure
- Zero-copy viewing" > "$SAMPLE_DIR/payload"

create_wasm "$SAMPLE_DIR/sync.wasm"

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/hello.sync" manifest.toml payload sync.wasm)

# ============================================
# Sample 2: CSV Data
# ============================================
echo "Creating data.sync..."
SAMPLE_DIR="$TEMP_DIR/data"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "text/csv"
display_ext = "csv"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 604800
timeout = 60

[permissions]
allow_hosts = []
allow_env = []

[ownership]
write_allowed = false

[verification]
enabled = false
EOF

cat > "$SAMPLE_DIR/payload" << 'EOF'
id,name,email,department,salary
1,Alice Johnson,alice@example.com,Engineering,85000
2,Bob Smith,bob@example.com,Marketing,72000
3,Carol Williams,carol@example.com,Engineering,92000
4,David Brown,david@example.com,Sales,68000
5,Eva Martinez,eva@example.com,Engineering,88000
6,Frank Lee,frank@example.com,HR,65000
7,Grace Chen,grace@example.com,Engineering,95000
8,Henry Wilson,henry@example.com,Marketing,74000
9,Ivy Thompson,ivy@example.com,Sales,71000
10,Jack Davis,jack@example.com,Engineering,89000
EOF

create_wasm "$SAMPLE_DIR/sync.wasm"

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/data.sync" manifest.toml payload sync.wasm)

# ============================================
# Sample 3: JSON Configuration
# ============================================
echo "Creating config.sync..."
SAMPLE_DIR="$TEMP_DIR/config"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "application/json"
display_ext = "json"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30

[permissions]
allow_hosts = ["api.example.com"]
allow_env = ["NODE_ENV"]

[ownership]
owner_capsule = "capsule://demo/config"
write_allowed = true

[verification]
enabled = true
vm_type = "wasm"
proof_type = "zk-snark"
EOF

cat > "$SAMPLE_DIR/payload" << 'EOF'
{
  "app": {
    "name": "Sync Demo Application",
    "version": "2.1.0",
    "environment": "production"
  },
  "database": {
    "host": "db.example.com",
    "port": 5432,
    "pool_size": 10,
    "ssl": true
  },
  "features": {
    "dark_mode": true,
    "notifications": true,
    "beta_features": false,
    "max_upload_size_mb": 100
  },
  "api": {
    "rate_limit": 1000,
    "timeout_seconds": 30,
    "endpoints": [
      "/api/v1/users",
      "/api/v1/data",
      "/api/v1/sync"
    ]
  },
  "logging": {
    "level": "info",
    "format": "json",
    "destinations": ["stdout", "file"]
  }
}
EOF

create_wasm "$SAMPLE_DIR/sync.wasm"

# Also add context.json for this one
cat > "$SAMPLE_DIR/context.json" << 'EOF'
{
  "last_updated": "2026-02-02T12:00:00Z",
  "update_source": "config-service",
  "checksum": "abc123"
}
EOF

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/config.sync" manifest.toml payload sync.wasm context.json)

# ============================================
# Sample 4: SVG Image
# ============================================
echo "Creating image.sync..."
SAMPLE_DIR="$TEMP_DIR/image"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "image/svg+xml"
display_ext = "svg"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 2592000
timeout = 30

[permissions]
allow_hosts = []
allow_env = []

[ownership]
write_allowed = false

[verification]
enabled = false
EOF

cat > "$SAMPLE_DIR/payload" << 'EOF'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 200">
  <defs>
    <linearGradient id="grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" style="stop-color:#667eea;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#764ba2;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Background -->
  <rect width="200" height="200" rx="20" fill="url(#grad)"/>
  
  <!-- Sync symbol -->
  <g transform="translate(100, 100)">
    <!-- Outer ring -->
    <circle cx="0" cy="0" r="60" fill="none" stroke="white" stroke-width="4" opacity="0.3"/>
    
    <!-- Arrow 1 -->
    <path d="M -40 0 A 40 40 0 0 1 40 0" fill="none" stroke="white" stroke-width="6" stroke-linecap="round"/>
    <polygon points="35,-10 45,0 35,10" fill="white"/>
    
    <!-- Arrow 2 -->
    <path d="M 40 0 A 40 40 0 0 1 -40 0" fill="none" stroke="white" stroke-width="6" stroke-linecap="round"/>
    <polygon points="-35,10 -45,0 -35,-10" fill="white"/>
    
    <!-- Center dot -->
    <circle cx="0" cy="0" r="8" fill="white"/>
  </g>
  
  <!-- Text -->
  <text x="100" y="180" text-anchor="middle" fill="white" font-family="system-ui" font-size="14" font-weight="bold">.sync</text>
</svg>
EOF

create_wasm "$SAMPLE_DIR/sync.wasm"

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/image.sync" manifest.toml payload sync.wasm)

# ============================================
# Sample 5: Markdown Document
# ============================================
echo "Creating readme.sync..."
SAMPLE_DIR="$TEMP_DIR/readme"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "text/markdown"
display_ext = "md"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 86400
timeout = 30

[permissions]
allow_hosts = []
allow_env = []

[ownership]
write_allowed = false

[verification]
enabled = false
EOF

cat > "$SAMPLE_DIR/payload" << 'EOF'
# Welcome to .sync Format

This is a **Markdown** document stored in a `.sync` archive.

## What is .sync?

The `.sync` format is a self-updating archive that combines:

- 📦 **Zero-Copy Data Access** - Instant file operations
- 🔄 **Self-Updating Logic** - Embedded WASM modules
- 🔒 **Sandboxed Execution** - Policy-driven permissions
- 📁 **ZIP-Compatible** - Standard container format

## Structure

```
file.sync
├── manifest.toml    # Metadata and policies
├── payload          # Your data (uncompressed)
├── sync.wasm        # Update logic (optional)
└── context.json     # Runtime context (optional)
```

## Use Cases

1. **Offline-First Apps** - Data with built-in refresh logic
2. **Edge Computing** - Portable, self-contained packages
3. **Secure Data Sharing** - Verified, policy-controlled access

---

*Created with sync-rs Web Viewer*
EOF

create_wasm "$SAMPLE_DIR/sync.wasm"

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/readme.sync" manifest.toml payload sync.wasm)

# ============================================
# Sample 6: HTML Widget
# ============================================
echo "Creating widget.sync..."
SAMPLE_DIR="$TEMP_DIR/widget"
mkdir -p "$SAMPLE_DIR"

cat > "$SAMPLE_DIR/manifest.toml" << 'EOF'
[sync]
version = "1.0"
content_type = "text/html"
display_ext = "html"

[meta]
created_by = "web-viewer-samples"
created_at = "2026-02-02T12:00:00Z"
hash_algo = "blake3"

[policy]
ttl = 3600
timeout = 30

[permissions]
allow_hosts = []
allow_env = []

[ownership]
write_allowed = false

[verification]
enabled = false
EOF

cat > "$SAMPLE_DIR/payload" << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      background: linear-gradient(135deg, #1e3a5f 0%, #0d1b2a 100%);
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      color: white;
    }
    .card {
      background: rgba(255, 255, 255, 0.1);
      backdrop-filter: blur(10px);
      border-radius: 16px;
      padding: 32px;
      max-width: 400px;
      text-align: center;
      border: 1px solid rgba(255, 255, 255, 0.2);
    }
    h1 { font-size: 24px; margin-bottom: 16px; }
    p { opacity: 0.8; line-height: 1.6; margin-bottom: 24px; }
    .stats {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 16px;
    }
    .stat { text-align: center; }
    .stat-value { font-size: 28px; font-weight: bold; color: #60a5fa; }
    .stat-label { font-size: 12px; opacity: 0.6; margin-top: 4px; }
    .pulse {
      animation: pulse 2s ease-in-out infinite;
    }
    @keyframes pulse {
      0%, 100% { transform: scale(1); }
      50% { transform: scale(1.05); }
    }
  </style>
</head>
<body>
  <div class="card pulse">
    <h1>🔮 Sync Widget</h1>
    <p>This is an interactive HTML widget stored in a .sync archive. It demonstrates the ability to embed rich content.</p>
    <div class="stats">
      <div class="stat">
        <div class="stat-value">128</div>
        <div class="stat-label">Files</div>
      </div>
      <div class="stat">
        <div class="stat-value">4.2K</div>
        <div class="stat-label">Views</div>
      </div>
      <div class="stat">
        <div class="stat-value">99%</div>
        <div class="stat-label">Uptime</div>
      </div>
    </div>
  </div>
</body>
</html>
EOF

create_wasm "$SAMPLE_DIR/sync.wasm"

(cd "$SAMPLE_DIR" && zip -0 "$SAMPLES_DIR/widget.sync" manifest.toml payload sync.wasm)

# Cleanup
rm -rf "$TEMP_DIR"

echo ""
echo "✅ Created sample .sync files:"
ls -la "$SAMPLES_DIR"
