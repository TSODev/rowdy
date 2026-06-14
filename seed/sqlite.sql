-- =============================================================================
-- Rowdy test seed — SQLite
-- Usage : sqlite3 /path/to/test.db < seed/sqlite.sql
-- Replay: same command — drops and recreates everything cleanly
-- Requires SQLite >= 3.35 (WITH ... INSERT support, 2021-03-12)
-- =============================================================================

PRAGMA foreign_keys = OFF;

-- ── Drop (reverse dependency order) ──────────────────────────────────────────
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS authors;
DROP TABLE IF EXISTS categories;

PRAGMA foreign_keys = ON;

-- ── Schema ────────────────────────────────────────────────────────────────────

CREATE TABLE categories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT
);

CREATE TABLE authors (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    first_name  TEXT NOT NULL,
    last_name   TEXT NOT NULL,
    country     TEXT NOT NULL,
    birth_year  INTEGER
);

CREATE TABLE books (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    title          TEXT    NOT NULL,
    author_id      INTEGER NOT NULL REFERENCES authors(id),
    category_id    INTEGER NOT NULL REFERENCES categories(id),
    published_year INTEGER,
    price          REAL    NOT NULL,
    stock          INTEGER NOT NULL DEFAULT 0,
    available      INTEGER NOT NULL DEFAULT 1  -- 0/1 boolean
);

CREATE TABLE customers (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    first_name    TEXT NOT NULL,
    last_name     TEXT NOT NULL,
    email         TEXT NOT NULL UNIQUE,
    city          TEXT,
    registered_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE orders (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    customer_id INTEGER NOT NULL REFERENCES customers(id),
    ordered_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    status      TEXT    NOT NULL DEFAULT 'pending',
    total       REAL
);

CREATE TABLE order_items (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    order_id   INTEGER NOT NULL REFERENCES orders(id),
    book_id    INTEGER NOT NULL REFERENCES books(id),
    quantity   INTEGER NOT NULL DEFAULT 1,
    unit_price REAL    NOT NULL
);

-- ── Categories (6) ───────────────────────────────────────────────────────────

INSERT INTO categories (name, description) VALUES
    ('Fiction',      'Novels, short stories and literary fiction'),
    ('Science',      'Physics, biology, chemistry and natural sciences'),
    ('History',      'World history, biographies and historical analysis'),
    ('Technology',   'Programming, engineering and digital culture'),
    ('Philosophy',   'Ethics, metaphysics, logic and political thought'),
    ('Children',     'Picture books, middle-grade and young adult');

-- ── Authors (20) ─────────────────────────────────────────────────────────────

INSERT INTO authors (first_name, last_name, country, birth_year) VALUES
    ('George',     'Orwell',        'United Kingdom', 1903),
    ('Ursula',     'Le Guin',       'United States',  1929),
    ('Isaac',      'Asimov',        'United States',  1920),
    ('Chimamanda', 'Adichie',       'Nigeria',        1977),
    ('Haruki',     'Murakami',      'Japan',          1949),
    ('Gabriel',    'García Márquez','Colombia',       1927),
    ('Toni',       'Morrison',      'United States',  1931),
    ('Franz',      'Kafka',         'Czech Republic', 1883),
    ('Virginia',   'Woolf',         'United Kingdom', 1882),
    ('Albert',     'Camus',         'France',         1913),
    ('Fyodor',     'Dostoevsky',    'Russia',         1821),
    ('Leo',        'Tolstoy',       'Russia',         1828),
    ('Jane',       'Austen',        'United Kingdom', 1775),
    ('Mark',       'Twain',         'United States',  1835),
    ('Emily',      'Dickinson',     'United States',  1830),
    ('Jorge',      'Borges',        'Argentina',      1899),
    ('Simone',     'de Beauvoir',   'France',         1908),
    ('Yukio',      'Mishima',       'Japan',          1925),
    ('Chinua',     'Achebe',        'Nigeria',        1930),
    ('Milan',      'Kundera',       'Czech Republic', 1929);

-- ── Books (300) ───────────────────────────────────────────────────────────────

WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 300
),
adj(n, word) AS (VALUES
    (0,'The Great'),(1,'Lost'),(2,'Dark'),(3,'Hidden'),(4,'Eternal'),
    (5,'Sacred'),(6,'Ancient'),(7,'Modern'),(8,'Silent'),(9,'Golden')
),
noun(n, word) AS (VALUES
    (0,'Journey'),(1,'Mystery'),(2,'Empire'),(3,'Legacy'),(4,'Chronicles'),
    (5,'Saga'),(6,'Quest'),(7,'Secret'),(8,'Voyage'),(9,'Dream')
)
INSERT INTO books (title, author_id, category_id, published_year, price, stock, available)
SELECT
    adj.word || ' ' || noun.word || ' — vol. ' || seq.n,
    (seq.n % 20) + 1,
    (seq.n % 6)  + 1,
    1950 + (seq.n % 74),
    ROUND(9.99 + (seq.n % 50) * 0.70, 2),
    (seq.n * 3) % 40,
    CASE WHEN seq.n % 8 = 0 THEN 0 ELSE 1 END
FROM seq
JOIN adj  ON adj.n  = seq.n % 10
JOIN noun ON noun.n = seq.n % 10;

-- ── Customers (50) ───────────────────────────────────────────────────────────

WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 50
),
fn(n, word) AS (VALUES
    (0,'Alice'),(1,'Bob'),(2,'Clara'),(3,'David'),(4,'Eva'),
    (5,'Frank'),(6,'Grace'),(7,'Henry'),(8,'Irene'),(9,'Jack')
),
ln(n, word) AS (VALUES
    (0,'Smith'),(1,'Jones'),(2,'Martin'),(3,'Brown'),(4,'Wilson'),
    (5,'Taylor'),(6,'Anderson'),(7,'Thomas'),(8,'Jackson'),(9,'White')
),
ct(n, word) AS (VALUES
    (0,'Paris'),(1,'London'),(2,'Berlin'),(3,'Tokyo'),(4,'New York'),
    (5,'Sydney'),(6,'Toronto'),(7,'Lagos'),(8,'Buenos Aires'),(9,'Seoul')
)
INSERT INTO customers (first_name, last_name, email, city, registered_at)
SELECT
    fn.word,
    ln.word,
    lower(fn.word) || '.' || lower(ln.word) || seq.n || '@example.com',
    ct.word,
    datetime('now', '-' || (seq.n * 7) || ' days')
FROM seq
JOIN fn ON fn.n = seq.n % 10
JOIN ln ON ln.n = seq.n % 10
JOIN ct ON ct.n = seq.n % 10;

-- ── Orders (300 — 6 per customer) ────────────────────────────────────────────

WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 300
),
st(n, word) AS (VALUES
    (0,'pending'),(1,'shipped'),(2,'delivered'),
    (3,'cancelled'),(4,'pending'),(5,'shipped')
)
INSERT INTO orders (customer_id, ordered_at, status)
SELECT
    ((seq.n - 1) % 50) + 1,
    datetime('now', '-' || (seq.n * 2) || ' days'),
    st.word
FROM seq
JOIN st ON st.n = seq.n % 6;

-- ── Order items (2 per order → 600 rows) ─────────────────────────────────────

WITH RECURSIVE
item(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM item WHERE n < 2)
INSERT INTO order_items (order_id, book_id, quantity, unit_price)
SELECT
    o.id,
    ((o.id + item.n - 2) % 300) + 1,
    (o.id % 3) + 1,
    b.price
FROM orders o
CROSS JOIN item
JOIN books b ON b.id = ((o.id + item.n - 2) % 300) + 1;

-- ── Update order totals ───────────────────────────────────────────────────────

UPDATE orders
SET total = (
    SELECT SUM(oi.quantity * oi.unit_price)
    FROM order_items oi
    WHERE oi.order_id = orders.id
);

-- ── Summary ───────────────────────────────────────────────────────────────────

SELECT 'categories'  AS tbl, COUNT(*) AS rows FROM categories
UNION ALL SELECT 'authors',     COUNT(*) FROM authors
UNION ALL SELECT 'books',       COUNT(*) FROM books
UNION ALL SELECT 'customers',   COUNT(*) FROM customers
UNION ALL SELECT 'orders',      COUNT(*) FROM orders
UNION ALL SELECT 'order_items', COUNT(*) FROM order_items;
