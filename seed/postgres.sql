-- =============================================================================
-- Rowdy test seed — PostgreSQL
-- Usage : psql -U <user> -d <dbname> -f seed/postgres.sql
-- Replay: same command — drops and recreates everything cleanly
-- =============================================================================

\set ON_ERROR_STOP on

-- ── Drop (reverse dependency order) ──────────────────────────────────────────
DROP TABLE IF EXISTS order_items CASCADE;
DROP TABLE IF EXISTS orders       CASCADE;
DROP TABLE IF EXISTS customers    CASCADE;
DROP TABLE IF EXISTS books        CASCADE;
DROP TABLE IF EXISTS authors      CASCADE;
DROP TABLE IF EXISTS categories   CASCADE;

-- ── Schema ────────────────────────────────────────────────────────────────────

CREATE TABLE categories (
    id          SERIAL PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT
);

CREATE TABLE authors (
    id          SERIAL PRIMARY KEY,
    first_name  TEXT NOT NULL,
    last_name   TEXT NOT NULL,
    country     TEXT NOT NULL,
    birth_year  INT
);

CREATE TABLE books (
    id             SERIAL PRIMARY KEY,
    title          TEXT         NOT NULL,
    author_id      INT          NOT NULL REFERENCES authors(id),
    category_id    INT          NOT NULL REFERENCES categories(id),
    published_year INT,
    price          NUMERIC(8,2) NOT NULL,
    stock          INT          NOT NULL DEFAULT 0,
    available      BOOLEAN      NOT NULL DEFAULT TRUE
);

CREATE TABLE customers (
    id            SERIAL PRIMARY KEY,
    first_name    TEXT        NOT NULL,
    last_name     TEXT        NOT NULL,
    email         TEXT        NOT NULL UNIQUE,
    city          TEXT,
    registered_at TIMESTAMP   NOT NULL DEFAULT NOW()
);

CREATE TABLE orders (
    id          SERIAL PRIMARY KEY,
    customer_id INT          NOT NULL REFERENCES customers(id),
    ordered_at  TIMESTAMP    NOT NULL DEFAULT NOW(),
    status      TEXT         NOT NULL DEFAULT 'pending',
    total       NUMERIC(10,2)
);

CREATE TABLE order_items (
    id         SERIAL PRIMARY KEY,
    order_id   INT          NOT NULL REFERENCES orders(id),
    book_id    INT          NOT NULL REFERENCES books(id),
    quantity   INT          NOT NULL DEFAULT 1,
    unit_price NUMERIC(8,2) NOT NULL
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
    ('George',     'Orwell',       'United Kingdom', 1903),
    ('Ursula',     'Le Guin',      'United States',  1929),
    ('Isaac',      'Asimov',       'United States',  1920),
    ('Chimamanda', 'Adichie',      'Nigeria',        1977),
    ('Haruki',     'Murakami',     'Japan',          1949),
    ('Gabriel',    'García Márquez','Colombia',      1927),
    ('Toni',       'Morrison',     'United States',  1931),
    ('Franz',      'Kafka',        'Czech Republic', 1883),
    ('Virginia',   'Woolf',        'United Kingdom', 1882),
    ('Albert',     'Camus',        'France',         1913),
    ('Dostoevsky', 'Fyodor',       'Russia',         1821),
    ('Leo',        'Tolstoy',      'Russia',         1828),
    ('Jane',       'Austen',       'United Kingdom', 1775),
    ('Mark',       'Twain',        'United States',  1835),
    ('Emily',      'Dickinson',    'United States',  1830),
    ('Jorge',      'Borges',       'Argentina',      1899),
    ('Simone',     'de Beauvoir',  'France',         1908),
    ('Yukio',      'Mishima',      'Japan',          1925),
    ('Chinua',     'Achebe',       'Nigeria',        1930),
    ('Milan',      'Kundera',      'Czech Republic', 1929);

-- ── Books (300) ───────────────────────────────────────────────────────────────
-- Titles built from adjective × noun grids to maximise filter variety

INSERT INTO books (title, author_id, category_id, published_year, price, stock, available)
SELECT
    adj.word || ' ' || noun.word || ' — vol. ' || s           AS title,
    (s % 20) + 1                                               AS author_id,
    (s % 6)  + 1                                               AS category_id,
    1950 + (s % 74)                                            AS published_year,
    ROUND((9.99 + (s % 50) * 0.70)::numeric, 2)               AS price,
    (s * 3) % 40                                               AS stock,
    (s % 8 <> 0)                                               AS available
FROM generate_series(1, 300) AS s
CROSS JOIN LATERAL (
    SELECT (ARRAY[
        'The Great','Lost','Dark','Hidden','Eternal',
        'Sacred','Ancient','Modern','Silent','Golden'
    ])[ (s % 10) + 1 ] AS word
) AS adj
CROSS JOIN LATERAL (
    SELECT (ARRAY[
        'Journey','Mystery','Empire','Legacy','Chronicles',
        'Saga','Quest','Secret','Voyage','Dream'
    ])[ (s % 10) + 1 ] AS word
) AS noun;

-- ── Customers (50) ───────────────────────────────────────────────────────────

INSERT INTO customers (first_name, last_name, email, city, registered_at)
SELECT
    fn.word                                                          AS first_name,
    ln.word                                                          AS last_name,
    lower(fn.word) || '.' || lower(ln.word) || s || '@example.com'  AS email,
    city.word                                                        AS city,
    NOW() - (s * 7 || ' days')::interval                            AS registered_at
FROM generate_series(1, 50) AS s
CROSS JOIN LATERAL (
    SELECT (ARRAY[
        'Alice','Bob','Clara','David','Eva',
        'Frank','Grace','Henry','Irene','Jack'
    ])[ (s % 10) + 1 ] AS word
) AS fn
CROSS JOIN LATERAL (
    SELECT (ARRAY[
        'Smith','Jones','Martin','Brown','Wilson',
        'Taylor','Anderson','Thomas','Jackson','White'
    ])[ (s % 10) + 1 ] AS word
) AS ln
CROSS JOIN LATERAL (
    SELECT (ARRAY[
        'Paris','London','Berlin','Tokyo','New York',
        'Sydney','Toronto','Lagos','Buenos Aires','Seoul'
    ])[ (s % 10) + 1 ] AS word
) AS city;

-- ── Orders (300 — 6 per customer) ────────────────────────────────────────────

INSERT INTO orders (customer_id, ordered_at, status, total)
SELECT
    ((s - 1) % 50) + 1                                          AS customer_id,
    NOW() - (s * 2 || ' days')::interval                        AS ordered_at,
    (ARRAY['pending','shipped','delivered','cancelled','pending','shipped'])[ (s % 6) + 1 ] AS status,
    NULL                                                          AS total
FROM generate_series(1, 300) AS s;

-- ── Order items (2 per order → 600 rows) ─────────────────────────────────────

INSERT INTO order_items (order_id, book_id, quantity, unit_price)
SELECT
    o.id                          AS order_id,
    ((o.id + item - 2) % 300) + 1 AS book_id,
    (o.id % 3) + 1                AS quantity,
    b.price                       AS unit_price
FROM orders o
CROSS JOIN generate_series(1, 2) AS item
JOIN books b ON b.id = ((o.id + item - 2) % 300) + 1;

-- ── Update order totals ───────────────────────────────────────────────────────

UPDATE orders o
SET total = (
    SELECT SUM(oi.quantity * oi.unit_price)
    FROM order_items oi
    WHERE oi.order_id = o.id
);

-- ── Summary ───────────────────────────────────────────────────────────────────

SELECT 'categories' AS "table", COUNT(*) AS rows FROM categories
UNION ALL SELECT 'authors',    COUNT(*) FROM authors
UNION ALL SELECT 'books',      COUNT(*) FROM books
UNION ALL SELECT 'customers',  COUNT(*) FROM customers
UNION ALL SELECT 'orders',     COUNT(*) FROM orders
UNION ALL SELECT 'order_items',COUNT(*) FROM order_items;
