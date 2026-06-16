#!/usr/bin/env bash
# =============================================================================
# Rowdy test seed — Redis
# Usage  : bash seed/redis.sh [host] [port]
#          bash seed/redis.sh                    → localhost:6379
#          bash seed/redis.sh redis.example.com  → remote:6379
#          bash seed/redis.sh 127.0.0.1 6380     → custom port
# Replay : same command — flushes DB 0 and recreates everything cleanly
# Requires: redis-cli in PATH
# =============================================================================

HOST="${1:-127.0.0.1}"
PORT="${2:-6379}"
CLI="redis-cli -h $HOST -p $PORT"

# ── Connectivity check ────────────────────────────────────────────────────────

if ! $CLI PING | grep -q PONG; then
    echo "ERROR: cannot reach Redis at $HOST:$PORT"
    exit 1
fi

echo "Connected to Redis at $HOST:$PORT"

# ── Flush ─────────────────────────────────────────────────────────────────────

$CLI FLUSHDB
echo "Flushed DB 0"

# =============================================================================
# STRINGs
# =============================================================================

echo ""
echo "── Strings ───────────────────────────────────────────────────────────────"

# App config (permanent)
$CLI SET  config:app:name          "Rowdy Bookstore"
$CLI SET  config:app:version       "3.2.1"
$CLI SET  config:app:base_url      "https://bookstore.example.com"
$CLI SET  config:app:timezone      "Europe/Paris"
$CLI SET  config:app:currency      "EUR"
$CLI SET  config:mail:smtp_host    "smtp.example.com"
$CLI SET  config:mail:smtp_port    "587"
$CLI SET  config:mail:from_address "no-reply@bookstore.example.com"
$CLI SET  config:features:reviews  "true"
$CLI SET  config:features:wishlist "true"
$CLI SET  config:features:reco     "false"

# Counters (atomic integers)
$CLI SET  counter:books:total       "300"
$CLI SET  counter:authors:total     "20"
$CLI SET  counter:orders:today      "42"
$CLI SET  counter:orders:pending    "8"
$CLI SET  counter:customers:total   "50"
$CLI SET  counter:customers:online  "7"
$CLI SET  counter:visits:today      "1284"
$CLI SET  counter:revenue:today     "3872.50"

# Feature flags
$CLI SET  flag:maintenance          "false"
$CLI SET  flag:new_checkout         "true"
$CLI SET  flag:dark_mode_default    "false"

# Session tokens (with TTL — simulates active user sessions)
$CLI SET  session:a1b2c3d4  "user_id:12"
$CLI EXPIRE session:a1b2c3d4  3600
$CLI SET  session:e5f6g7h8  "user_id:5"
$CLI EXPIRE session:e5f6g7h8  7200
$CLI SET  session:i9j0k1l2  "user_id:31"
$CLI EXPIRE session:i9j0k1l2  1800
$CLI SET  session:m3n4o5p6  "user_id:8"
$CLI EXPIRE session:m3n4o5p6  900

# Short-lived caches (TTL 5 min)
$CLI SET  cache:book:1:title    "The Great Journey — vol. 1"
$CLI EXPIRE cache:book:1:title  300
$CLI SET  cache:book:2:title    "Lost Mystery — vol. 2"
$CLI EXPIRE cache:book:2:title  300
$CLI SET  cache:homepage:html   "<html><!-- cached homepage --></html>"
$CLI EXPIRE cache:homepage:html 120

# API rate-limit counters (very short TTL)
$CLI SET  ratelimit:ip:192.168.1.10  "23"
$CLI EXPIRE ratelimit:ip:192.168.1.10  60
$CLI SET  ratelimit:ip:10.0.0.55     "5"
$CLI EXPIRE ratelimit:ip:10.0.0.55   60

echo "  Strings: $($CLI DBSIZE) keys total so far"

# =============================================================================
# HASHes
# =============================================================================

echo ""
echo "── Hashes ────────────────────────────────────────────────────────────────"

# User profiles
$CLI HSET user:1   first_name Alice   last_name Smith    email alice.smith@example.com    city Paris        role admin    registered 2023-01-15
$CLI HSET user:2   first_name Bob     last_name Jones    email bob.jones@example.com      city London       role customer registered 2023-03-22
$CLI HSET user:3   first_name Clara   last_name Martin   email clara.martin@example.com   city Berlin       role customer registered 2023-06-10
$CLI HSET user:4   first_name David   last_name Brown    email david.brown@example.com    city Tokyo        role editor   registered 2022-11-05
$CLI HSET user:5   first_name Eva     last_name Wilson   email eva.wilson@example.com     city "New York"   role customer registered 2024-01-30
$CLI HSET user:6   first_name Frank   last_name Taylor   email frank.taylor@example.com   city Sydney       role customer registered 2023-09-14
$CLI HSET user:7   first_name Grace   last_name Anderson email grace.anderson@example.com city Toronto      role customer registered 2024-02-28
$CLI HSET user:8   first_name Henry   last_name Thomas   email henry.thomas@example.com   city Lagos        role editor   registered 2022-08-19

# Book metadata cache (denormalized for fast reads)
$CLI HSET book:1   title "The Great Journey — vol. 1"    author "George Orwell"      category Fiction      price 9.99   stock 27  available true  published_year 1951
$CLI HSET book:2   title "Lost Mystery — vol. 2"         author "Ursula Le Guin"     category Science      price 10.69  stock 6   available true  published_year 1952
$CLI HSET book:3   title "Dark Empire — vol. 3"          author "Isaac Asimov"       category Technology   price 11.39  stock 9   available true  published_year 1953
$CLI HSET book:4   title "Hidden Legacy — vol. 4"        author "Chimamanda Adichie" category History      price 12.09  stock 12  available true  published_year 1954
$CLI HSET book:5   title "Eternal Chronicles — vol. 5"   author "Haruki Murakami"    category Philosophy   price 12.79  stock 15  available false published_year 1955

# Order summaries
$CLI HSET order:1  customer_id 1  status delivered  total 32.47  ordered_at "2026-05-01 14:22:10"  items 3
$CLI HSET order:2  customer_id 2  status shipped    total 21.38  ordered_at "2026-05-15 09:05:33"  items 2
$CLI HSET order:3  customer_id 1  status pending    total 44.55  ordered_at "2026-06-10 18:41:02"  items 4
$CLI HSET order:4  customer_id 5  status cancelled  total 9.99   ordered_at "2026-06-12 11:15:50"  items 1

# Daily stats
$CLI HSET stats:2026-06-16  visits 1284  orders 42  revenue 3872.50  new_customers 3  returns 1
$CLI HSET stats:2026-06-15  visits 1103  orders 38  revenue 3451.20  new_customers 5  returns 2
$CLI HSET stats:2026-06-14  visits  992  orders 31  revenue 2890.10  new_customers 2  returns 0

# App settings (editable at runtime)
$CLI HSET settings:pagination  page_size 20   max_pages 50
$CLI HSET settings:search      min_chars 2    max_results 100  fuzzy true
$CLI HSET settings:uploads     max_size_mb 5  allowed_types "jpg,png,webp"

echo "  Hashes done"

# =============================================================================
# LISTs
# =============================================================================

echo ""
echo "── Lists ─────────────────────────────────────────────────────────────────"

# Order processing queue (FIFO — RPUSH to add, LPOP to consume)
$CLI DEL queue:orders:pending
$CLI RPUSH queue:orders:pending  103 107 112 98 145 201 88 233

# Error log (LIFO — newest first via LPUSH)
$CLI DEL log:errors:recent
$CLI LPUSH log:errors:recent \
    "[2026-06-16 18:42:01] WARN  payment gateway timeout order_id=233" \
    "[2026-06-16 17:15:33] ERROR db connection pool exhausted" \
    "[2026-06-16 14:08:55] WARN  image resize failed book_id=12" \
    "[2026-06-15 22:31:10] ERROR smtp auth failed" \
    "[2026-06-15 19:00:02] WARN  slow query 2340ms table=books"

# User search history (per user, LPUSH + LTRIM to keep last 10)
$CLI DEL history:search:user:1
$CLI LPUSH history:search:user:1 "kafka" "dystopia" "murakami" "jazz novel" "1984"
$CLI LTRIM history:search:user:1 0 9

$CLI DEL history:search:user:2
$CLI LPUSH history:search:user:2 "science fiction" "asimov" "robots" "foundation"
$CLI LTRIM history:search:user:2 0 9

# Recently viewed books (per user)
$CLI DEL recent:books:user:1
$CLI RPUSH recent:books:user:1  1 14 7 32 5 18

$CLI DEL recent:books:user:5
$CLI RPUSH recent:books:user:5  3 9 22 44

# Notification inbox (per user)
$CLI DEL notifications:user:1
$CLI RPUSH notifications:user:1 \
    "Your order #233 has been shipped" \
    "New book by Murakami is available" \
    "Your review was approved"

$CLI DEL notifications:user:3
$CLI RPUSH notifications:user:3 \
    "Your wishlist item is back in stock" \
    "Weekend sale: 20% off all Philosophy books"

# Scheduled task queue
$CLI DEL queue:tasks:scheduled
$CLI RPUSH queue:tasks:scheduled \
    "send_newsletter:2026-06-17T08:00:00" \
    "reindex_search:2026-06-17T02:00:00" \
    "generate_report:2026-06-17T06:00:00" \
    "cleanup_sessions:2026-06-17T03:00:00"

echo "  Lists done"

# =============================================================================
# SETs
# =============================================================================

echo ""
echo "── Sets ──────────────────────────────────────────────────────────────────"

# Book tags
$CLI DEL tags:book:1
$CLI SADD tags:book:1  fiction classic dystopia bestseller "20th-century"

$CLI DEL tags:book:2
$CLI SADD tags:book:2  "science-fiction" classic "space-opera" robots

$CLI DEL tags:book:3
$CLI SADD tags:book:3  technology programming "ai" futurism

$CLI DEL tags:book:4
$CLI SADD tags:book:4  "african-literature" contemporary "social-issues"

$CLI DEL tags:book:5
$CLI SADD tags:book:5  fiction "magical-realism" japan contemporary

# Active / online users
$CLI DEL users:online
$CLI SADD users:online  1 3 5 7 8

# Premium subscribers
$CLI DEL users:premium
$CLI SADD users:premium  1 4 5 8 12 17 23 31 44

# IP blocklist
$CLI DEL security:blocklist:ips
$CLI SADD security:blocklist:ips \
    "203.0.113.42" \
    "198.51.100.7" \
    "192.0.2.255"

# Categories with active promotions
$CLI DEL promos:active:categories
$CLI SADD promos:active:categories  1 4 5

# Books out of stock
$CLI DEL inventory:out_of_stock
$CLI SADD inventory:out_of_stock  8 16 24 32 40 48 56

# Supported languages
$CLI DEL config:supported:languages
$CLI SADD config:supported:languages  en fr de ja es pt "zh-CN"

echo "  Sets done"

# =============================================================================
# SORTED SETs
# =============================================================================

echo ""
echo "── Sorted sets ───────────────────────────────────────────────────────────"

# Bestsellers ranked by total sales
$CLI DEL ranking:bestsellers
$CLI ZADD ranking:bestsellers \
    842  "book:1:The Great Journey" \
    731  "book:7:Sacred Secret" \
    618  "book:13:Ancient Quest" \
    594  "book:3:Dark Empire" \
    512  "book:20:Golden Dream" \
    487  "book:9:Hidden Legacy" \
    455  "book:5:Eternal Chronicles" \
    412  "book:17:Modern Voyage" \
    389  "book:25:Silent Saga" \
    341  "book:11:Lost Mystery"

# Authors ranked by average review score (0–50 scale)
$CLI DEL ranking:authors:score
$CLI ZADD ranking:authors:score \
    48  "Haruki Murakami" \
    47  "Gabriel García Márquez" \
    46  "Toni Morrison" \
    45  "Ursula Le Guin" \
    44  "George Orwell" \
    43  "Isaac Asimov" \
    42  "Chimamanda Adichie" \
    41  "Franz Kafka" \
    40  "Albert Camus" \
    38  "Virginia Woolf"

# Priority task queue (score = unix timestamp of scheduled execution)
$CLI DEL queue:tasks:priority
$CLI ZADD queue:tasks:priority \
    1750118400  "reindex_search" \
    1750125600  "send_newsletter" \
    1750129200  "generate_monthly_report" \
    1750132800  "cleanup_expired_sessions" \
    1750136400  "backup_database" \
    1750140000  "sync_inventory"

# Customer loyalty points (score = points balance)
$CLI DEL loyalty:points
$CLI ZADD loyalty:points \
    4250  "user:1:Alice Smith" \
    3810  "user:5:Eva Wilson" \
    3120  "user:8:Henry Thomas" \
    2760  "user:4:David Brown" \
    2100  "user:2:Bob Jones" \
    1580  "user:7:Grace Anderson" \
    980   "user:3:Clara Martin" \
    450   "user:6:Frank Taylor"

# Trending search terms (score = search count last 24h)
$CLI DEL trending:searches
$CLI ZADD trending:searches \
    284  "murakami" \
    251  "1984" \
    198  "science fiction" \
    176  "kafka" \
    143  "robots" \
    121  "dystopia" \
    98   "philosophy" \
    87   "magical realism" \
    72   "japan" \
    61   "africa"

echo "  Sorted sets done"

# =============================================================================
# Summary
# =============================================================================

echo ""
echo "── Summary ───────────────────────────────────────────────────────────────"

TOTAL=$($CLI DBSIZE)
STRINGS=$($CLI KEYS '*' | xargs -I{} $CLI TYPE {} | grep -c "^string$" || true)
HASHES=$( $CLI KEYS '*' | xargs -I{} $CLI TYPE {} | grep -c "^hash$"   || true)
LISTS=$(  $CLI KEYS '*' | xargs -I{} $CLI TYPE {} | grep -c "^list$"   || true)
SETS=$(   $CLI KEYS '*' | xargs -I{} $CLI TYPE {} | grep -c "^set$"    || true)
ZSETS=$(  $CLI KEYS '*' | xargs -I{} $CLI TYPE {} | grep -c "^zset$"   || true)

echo "  DB 0 — total keys : $TOTAL"
printf "  %-14s %s\n" "string"  "$STRINGS"
printf "  %-14s %s\n" "hash"    "$HASHES"
printf "  %-14s %s\n" "list"    "$LISTS"
printf "  %-14s %s\n" "set"     "$SETS"
printf "  %-14s %s\n" "zset"    "$ZSETS"
echo ""
echo "Connect with: redis://127.0.0.1:6379"
echo "Done."
