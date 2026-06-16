#!/usr/bin/env bash
# Seed script for MongoDB Atlas demo data
# Usage: MONGODB_URI="mongodb+srv://user:pass@cluster/dbname?..." ./seed/mongodb.sh

set -euo pipefail

if [ -z "${MONGODB_URI:-}" ]; then
  echo "Error: MONGODB_URI environment variable is not set (or not exported)."
  echo ""
  echo "Option 1 — inline:  MONGODB_URI=\"mongodb+srv://...\" $0"
  echo "Option 2 — export:  export MONGODB_URI=\"mongodb+srv://...\" && $0"
  exit 1
fi

mongosh "$MONGODB_URI" --quiet <<'EOF'

// ── Drop existing collections ─────────────────────────────────────────────────
["users", "products", "orders", "reviews"].forEach(c => {
  try { db.getCollection(c).drop(); } catch (_) {}
});

// ── users ─────────────────────────────────────────────────────────────────────
// Showcases: nested address [obj], nested preferences [obj],
//            orders array of objects [arr:N], tags array of strings [arr:N]
db.users.insertMany([
  {
    _id: ObjectId("665000000000000000000001"),
    username: "alice_dev",
    email: "alice@example.com",
    age: 34,
    active: true,
    created_at: new Date("2024-01-15T09:00:00Z"),
    address: {
      street: "12 rue Lepic",
      city: "Paris",
      zip: "75018",
      country: "France",
      geo: { lat: 48.8867, lon: 2.3431 }
    },
    preferences: {
      theme: "dark",
      language: "fr",
      notifications: { email: true, sms: false, push: true }
    },
    tags: ["developer", "rust", "open-source"],
    recent_orders: [
      { order_id: "ORD-001", total: 89.99, status: "delivered" },
      { order_id: "ORD-007", total: 149.00, status: "shipped" }
    ]
  },
  {
    _id: ObjectId("665000000000000000000002"),
    username: "bob_ops",
    email: "bob@example.com",
    age: 28,
    active: true,
    created_at: new Date("2024-03-20T14:30:00Z"),
    address: {
      street: "5 Market Street",
      city: "London",
      zip: "EC1A 1BB",
      country: "UK",
      geo: { lat: 51.5134, lon: -0.0982 }
    },
    preferences: {
      theme: "light",
      language: "en",
      notifications: { email: true, sms: true, push: false }
    },
    tags: ["devops", "kubernetes", "monitoring"],
    recent_orders: [
      { order_id: "ORD-003", total: 299.50, status: "delivered" },
      { order_id: "ORD-012", total: 45.00, status: "processing" },
      { order_id: "ORD-015", total: 720.00, status: "pending" }
    ]
  },
  {
    _id: ObjectId("665000000000000000000003"),
    username: "carol_data",
    email: "carol@example.com",
    age: 41,
    active: false,
    created_at: new Date("2023-11-05T08:15:00Z"),
    address: {
      street: "88 Kurfürstendamm",
      city: "Berlin",
      zip: "10709",
      country: "Germany",
      geo: { lat: 52.5027, lon: 13.3290 }
    },
    preferences: {
      theme: "dark",
      language: "de",
      notifications: { email: false, sms: false, push: false }
    },
    tags: ["data-science", "python", "ml"],
    recent_orders: []
  },
  {
    _id: ObjectId("665000000000000000000004"),
    username: "dave_mobile",
    email: "dave@example.com",
    age: 25,
    active: true,
    created_at: new Date("2024-06-01T11:00:00Z"),
    address: {
      street: "Calle Gran Via 32",
      city: "Madrid",
      zip: "28013",
      country: "Spain",
      geo: { lat: 40.4201, lon: -3.7049 }
    },
    preferences: {
      theme: "auto",
      language: "es",
      notifications: { email: true, sms: false, push: true }
    },
    tags: ["mobile", "swift", "android"],
    recent_orders: [
      { order_id: "ORD-009", total: 55.00, status: "delivered" }
    ]
  },
  {
    _id: ObjectId("665000000000000000000005"),
    username: "eve_security",
    email: "eve@example.com",
    age: 37,
    active: true,
    created_at: new Date("2024-02-28T16:45:00Z"),
    address: {
      street: "Via Roma 10",
      city: "Milan",
      zip: "20121",
      country: "Italy",
      geo: { lat: 45.4654, lon: 9.1885 }
    },
    preferences: {
      theme: "dark",
      language: "it",
      notifications: { email: true, sms: true, push: true }
    },
    tags: ["security", "pentest", "network"],
    recent_orders: [
      { order_id: "ORD-002", total: 189.00, status: "delivered" },
      { order_id: "ORD-011", total: 340.00, status: "shipped" },
      { order_id: "ORD-018", total: 99.99, status: "delivered" },
      { order_id: "ORD-021", total: 500.00, status: "processing" }
    ]
  }
]);

// ── products ──────────────────────────────────────────────────────────────────
// Showcases: specs nested object, variants array of objects with their own
//            nested fields, categories array of strings
db.products.insertMany([
  {
    _id: ObjectId("665000000000000000000010"),
    sku: "LAPTOP-PRO-16",
    name: "ProBook 16 Laptop",
    brand: "TechCorp",
    price: 1299.99,
    in_stock: true,
    rating: 4.7,
    specs: {
      cpu: "Apple M3 Pro",
      ram_gb: 18,
      storage_gb: 512,
      display: { size_inch: 16.2, resolution: "3456x2234", refresh_hz: 120 },
      battery_wh: 100,
      weight_kg: 2.14
    },
    categories: ["computers", "laptops", "premium"],
    variants: [
      { color: "Silver", storage_gb: 512, price_delta: 0 },
      { color: "Space Black", storage_gb: 512, price_delta: 0 },
      { color: "Silver", storage_gb: 1024, price_delta: 200 }
    ],
    released_at: new Date("2024-01-08T00:00:00Z")
  },
  {
    _id: ObjectId("665000000000000000000011"),
    sku: "MECH-KB-TKL",
    name: "Mechanical TKL Keyboard",
    brand: "KeyMaster",
    price: 129.00,
    in_stock: true,
    rating: 4.5,
    specs: {
      layout: "TKL",
      switch_type: "Cherry MX Red",
      backlight: "RGB",
      connectivity: { usb: true, bluetooth: true, wireless_2_4g: true },
      key_count: 87,
      polling_hz: 1000
    },
    categories: ["peripherals", "keyboards", "mechanical"],
    variants: [
      { switch_type: "Cherry MX Red",   color: "Black", price_delta: 0   },
      { switch_type: "Cherry MX Blue",  color: "Black", price_delta: 0   },
      { switch_type: "Cherry MX Brown", color: "White", price_delta: 10  },
      { switch_type: "Cherry MX Speed", color: "Black", price_delta: 20  }
    ],
    released_at: new Date("2023-09-15T00:00:00Z")
  },
  {
    _id: ObjectId("665000000000000000000012"),
    sku: "MONITOR-4K-27",
    name: "CrystalView 27 4K",
    brand: "ViewTech",
    price: 549.00,
    in_stock: false,
    rating: 4.3,
    specs: {
      size_inch: 27,
      resolution: "3840x2160",
      panel: "IPS",
      refresh_hz: 144,
      hdr: true,
      ports: { hdmi: 2, displayport: 1, usb_c: 1, usb_hub: 3 },
      response_ms: 1
    },
    categories: ["monitors", "4k", "gaming"],
    variants: [
      { finish: "Matte", stand: "adjustable", price_delta: 0  },
      { finish: "Glossy", stand: "fixed",     price_delta: -30 }
    ],
    released_at: new Date("2024-04-10T00:00:00Z")
  },
  {
    _id: ObjectId("665000000000000000000013"),
    sku: "HEADSET-WL-PRO",
    name: "SoundPro Wireless Headset",
    brand: "AudioMax",
    price: 249.99,
    in_stock: true,
    rating: 4.8,
    specs: {
      driver_mm: 40,
      frequency_hz: { min: 20, max: 20000 },
      microphone: { type: "cardioid", noise_cancellation: true, detachable: true },
      battery_hours: 30,
      connectivity: { bluetooth: "5.2", usb_dongle: true, aux: true },
      weight_g: 295
    },
    categories: ["audio", "headsets", "wireless"],
    variants: [
      { color: "Black",  ear_pads: "leatherette" },
      { color: "White",  ear_pads: "leatherette" },
      { color: "Black",  ear_pads: "velour"      }
    ],
    released_at: new Date("2023-12-01T00:00:00Z")
  },
  {
    _id: ObjectId("665000000000000000000014"),
    sku: "DESK-SIT-STAND",
    name: "ErgoDesk Height-Adjustable",
    brand: "WorkBetter",
    price: 799.00,
    in_stock: true,
    rating: 4.6,
    specs: {
      surface_cm: { width: 160, depth: 80 },
      height_range_cm: { min: 62, max: 128 },
      motor: { type: "dual", noise_db: 45, speed_mm_s: 38 },
      max_load_kg: 100,
      memory_positions: 4,
      material: { top: "bamboo", frame: "steel" }
    },
    categories: ["furniture", "standing-desk", "ergonomic"],
    variants: [
      { top_color: "Natural Bamboo", frame_color: "Black", width_cm: 120 },
      { top_color: "Natural Bamboo", frame_color: "White", width_cm: 120 },
      { top_color: "Natural Bamboo", frame_color: "Black", width_cm: 160 },
      { top_color: "White",          frame_color: "White", width_cm: 160 }
    ],
    released_at: new Date("2024-02-14T00:00:00Z")
  }
]);

// ── orders ────────────────────────────────────────────────────────────────────
// Showcases: items array of objects (with nested product info),
//            shipping + payment nested objects, status history array
db.orders.insertMany([
  {
    _id: "ORD-001",
    user_id: ObjectId("665000000000000000000001"),
    status: "delivered",
    created_at: new Date("2024-05-10T10:22:00Z"),
    items: [
      { sku: "MECH-KB-TKL", name: "Mechanical TKL Keyboard", qty: 1, unit_price: 129.00 }
    ],
    shipping: {
      method: "express",
      carrier: "DHL",
      tracking: "1Z999AA10123456784",
      address: { street: "12 rue Lepic", city: "Paris", zip: "75018", country: "France" },
      estimated_at: new Date("2024-05-13T00:00:00Z"),
      delivered_at: new Date("2024-05-12T14:05:00Z")
    },
    payment: { method: "card", last4: "4242", currency: "EUR", total: 89.99, paid_at: new Date("2024-05-10T10:22:05Z") },
    status_history: [
      { status: "pending",    at: new Date("2024-05-10T10:22:00Z") },
      { status: "processing", at: new Date("2024-05-10T11:00:00Z") },
      { status: "shipped",    at: new Date("2024-05-11T08:30:00Z") },
      { status: "delivered",  at: new Date("2024-05-12T14:05:00Z") }
    ]
  },
  {
    _id: "ORD-002",
    user_id: ObjectId("665000000000000000000005"),
    status: "delivered",
    created_at: new Date("2024-05-18T16:40:00Z"),
    items: [
      { sku: "HEADSET-WL-PRO", name: "SoundPro Wireless Headset", qty: 1, unit_price: 249.99 }
    ],
    shipping: {
      method: "standard",
      carrier: "GLS",
      tracking: "GLSFR1234567890",
      address: { street: "Via Roma 10", city: "Milan", zip: "20121", country: "Italy" },
      estimated_at: new Date("2024-05-23T00:00:00Z"),
      delivered_at: new Date("2024-05-22T11:10:00Z")
    },
    payment: { method: "paypal", currency: "EUR", total: 189.00, paid_at: new Date("2024-05-18T16:40:12Z") },
    status_history: [
      { status: "pending",    at: new Date("2024-05-18T16:40:00Z") },
      { status: "shipped",    at: new Date("2024-05-19T09:00:00Z") },
      { status: "delivered",  at: new Date("2024-05-22T11:10:00Z") }
    ]
  },
  {
    _id: "ORD-003",
    user_id: ObjectId("665000000000000000000002"),
    status: "delivered",
    created_at: new Date("2024-06-01T09:15:00Z"),
    items: [
      { sku: "MECH-KB-TKL",   name: "Mechanical TKL Keyboard",  qty: 1, unit_price: 129.00 },
      { sku: "HEADSET-WL-PRO", name: "SoundPro Wireless Headset", qty: 1, unit_price: 249.99 }
    ],
    shipping: {
      method: "express",
      carrier: "UPS",
      tracking: "1Z999AA10123456999",
      address: { street: "5 Market Street", city: "London", zip: "EC1A 1BB", country: "UK" },
      estimated_at: new Date("2024-06-04T00:00:00Z"),
      delivered_at: new Date("2024-06-03T15:30:00Z")
    },
    payment: { method: "card", last4: "1337", currency: "GBP", total: 299.50, paid_at: new Date("2024-06-01T09:15:30Z") },
    status_history: [
      { status: "pending",    at: new Date("2024-06-01T09:15:00Z") },
      { status: "processing", at: new Date("2024-06-01T10:00:00Z") },
      { status: "shipped",    at: new Date("2024-06-02T08:00:00Z") },
      { status: "delivered",  at: new Date("2024-06-03T15:30:00Z") }
    ]
  },
  {
    _id: "ORD-007",
    user_id: ObjectId("665000000000000000000001"),
    status: "shipped",
    created_at: new Date("2024-06-12T14:00:00Z"),
    items: [
      { sku: "MONITOR-4K-27", name: "CrystalView 27 4K", qty: 1, unit_price: 549.00 }
    ],
    shipping: {
      method: "freight",
      carrier: "Colis Privé",
      tracking: "CP987654321FR",
      address: { street: "12 rue Lepic", city: "Paris", zip: "75018", country: "France" },
      estimated_at: new Date("2024-06-17T00:00:00Z"),
      delivered_at: null
    },
    payment: { method: "card", last4: "4242", currency: "EUR", total: 149.00, paid_at: new Date("2024-06-12T14:00:45Z") },
    status_history: [
      { status: "pending",    at: new Date("2024-06-12T14:00:00Z") },
      { status: "processing", at: new Date("2024-06-12T15:00:00Z") },
      { status: "shipped",    at: new Date("2024-06-13T07:45:00Z") }
    ]
  },
  {
    _id: "ORD-009",
    user_id: ObjectId("665000000000000000000004"),
    status: "delivered",
    created_at: new Date("2024-06-05T08:30:00Z"),
    items: [
      { sku: "MECH-KB-TKL", name: "Mechanical TKL Keyboard", qty: 1, unit_price: 129.00 }
    ],
    shipping: {
      method: "standard",
      carrier: "Correos",
      tracking: "ES123456789ES",
      address: { street: "Calle Gran Via 32", city: "Madrid", zip: "28013", country: "Spain" },
      estimated_at: new Date("2024-06-10T00:00:00Z"),
      delivered_at: new Date("2024-06-09T12:00:00Z")
    },
    payment: { method: "card", last4: "9876", currency: "EUR", total: 55.00, paid_at: new Date("2024-06-05T08:30:10Z") },
    status_history: [
      { status: "pending",   at: new Date("2024-06-05T08:30:00Z") },
      { status: "shipped",   at: new Date("2024-06-06T10:00:00Z") },
      { status: "delivered", at: new Date("2024-06-09T12:00:00Z") }
    ]
  }
]);

// ── reviews ───────────────────────────────────────────────────────────────────
// Showcases: sentiment nested object, helpful_votes array of ObjectIds,
//            media array of objects
db.reviews.insertMany([
  {
    product_sku: "LAPTOP-PRO-16",
    user_id: ObjectId("665000000000000000000001"),
    rating: 5,
    title: "Absolutely love it",
    body: "Best laptop I've ever owned. Fast, silent, great display.",
    verified_purchase: true,
    created_at: new Date("2024-03-10T20:00:00Z"),
    sentiment: { score: 0.95, label: "positive", keywords: ["fast", "silent", "display"] },
    media: [
      { type: "image", url: "https://cdn.example.com/r/001.jpg", caption: "Setup photo" },
      { type: "image", url: "https://cdn.example.com/r/002.jpg", caption: "Screen close-up" }
    ],
    helpful_votes: [
      ObjectId("665000000000000000000002"),
      ObjectId("665000000000000000000005")
    ]
  },
  {
    product_sku: "HEADSET-WL-PRO",
    user_id: ObjectId("665000000000000000000005"),
    rating: 5,
    title: "Perfect for long sessions",
    body: "30 hours battery is no joke. Mic is crystal clear.",
    verified_purchase: true,
    created_at: new Date("2024-05-25T18:30:00Z"),
    sentiment: { score: 0.92, label: "positive", keywords: ["battery", "mic", "clear"] },
    media: [],
    helpful_votes: [
      ObjectId("665000000000000000000001"),
      ObjectId("665000000000000000000003"),
      ObjectId("665000000000000000000004")
    ]
  },
  {
    product_sku: "MONITOR-4K-27",
    user_id: ObjectId("665000000000000000000002"),
    rating: 3,
    title: "Good but runs hot",
    body: "Image quality is excellent but the unit runs quite warm after 2 hours.",
    verified_purchase: false,
    created_at: new Date("2024-06-08T11:00:00Z"),
    sentiment: { score: 0.45, label: "mixed", keywords: ["quality", "warm", "hot"] },
    media: [
      { type: "video", url: "https://cdn.example.com/r/003.mp4", caption: "Thermal reading" }
    ],
    helpful_votes: [ObjectId("665000000000000000000005")]
  },
  {
    product_sku: "MECH-KB-TKL",
    user_id: ObjectId("665000000000000000000003"),
    rating: 4,
    title: "Solid keyboard, loud switches",
    body: "Very tactile and precise. The Blue switches are satisfying but loud — not for open offices.",
    verified_purchase: true,
    created_at: new Date("2024-04-02T09:45:00Z"),
    sentiment: { score: 0.70, label: "positive", keywords: ["tactile", "precise", "loud"] },
    media: [],
    helpful_votes: []
  },
  {
    product_sku: "DESK-SIT-STAND",
    user_id: ObjectId("665000000000000000000004"),
    rating: 5,
    title: "Changed my workday",
    body: "Easy to assemble, very stable. The bamboo top looks and feels premium.",
    verified_purchase: true,
    created_at: new Date("2024-06-10T14:20:00Z"),
    sentiment: { score: 0.97, label: "positive", keywords: ["stable", "bamboo", "premium"] },
    media: [
      { type: "image", url: "https://cdn.example.com/r/004.jpg", caption: "Full desk setup" },
      { type: "image", url: "https://cdn.example.com/r/005.jpg", caption: "Motor controls" },
      { type: "image", url: "https://cdn.example.com/r/006.jpg", caption: "Bamboo surface" }
    ],
    helpful_votes: [
      ObjectId("665000000000000000000001"),
      ObjectId("665000000000000000000002")
    ]
  }
]);

// ── Summary ───────────────────────────────────────────────────────────────────
print("\n✅ Seed complete:");
print("  users    : " + db.users.countDocuments());
print("  products : " + db.products.countDocuments());
print("  orders   : " + db.orders.countDocuments());
print("  reviews  : " + db.reviews.countDocuments());
print("\nCollections demonstrate:");
print("  [obj]   → address, preferences, specs, shipping, payment, sentiment");
print("  [arr:N] → tags, variants, items, status_history, media, helpful_votes");
EOF
