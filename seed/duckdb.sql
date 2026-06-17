-- =============================================================================
-- Rowdy test seed — DuckDB
-- Thème : librairie + analytique (cohérent avec les autres seeds)
-- Usage :
--   duckdb /tmp/rowdy_test.duckdb < seed/duckdb.sql
-- Replay (idempotent) :
--   rm -f /tmp/rowdy_test.duckdb && duckdb /tmp/rowdy_test.duckdb < seed/duckdb.sql
-- Connexion dans Rowdy :
--   type : duckdb
--   url  : duckdb:///tmp/rowdy_test.duckdb
-- =============================================================================

-- ── Drop (reverse dependency order) ──────────────────────────────────────────
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS book_tags;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS authors;
DROP TABLE IF EXISTS categories;
DROP TABLE IF EXISTS daily_sales;

-- ── Schema ────────────────────────────────────────────────────────────────────

CREATE TABLE categories (
    id          INTEGER PRIMARY KEY,
    name        VARCHAR NOT NULL,
    description VARCHAR
);

CREATE TABLE authors (
    id          INTEGER PRIMARY KEY,
    first_name  VARCHAR NOT NULL,
    last_name   VARCHAR NOT NULL,
    country     VARCHAR NOT NULL,
    birth_year  INTEGER,
    -- DuckDB LIST type : langues parlées par l'auteur
    languages   VARCHAR[]
);

-- Note: DuckDB v1.x enforces FK constraints via DELETE+INSERT on complex types
-- (VARCHAR[], STRUCT), making UPDATE impossible on parent rows that have children.
-- FK constraints are therefore omitted; relationships are documented by column names.
CREATE TABLE books (
    id             INTEGER PRIMARY KEY,
    title          VARCHAR NOT NULL,
    author_id      INTEGER NOT NULL,   -- → authors.id
    category_id    INTEGER NOT NULL,   -- → categories.id
    published_year INTEGER,
    price          DECIMAL(8, 2) NOT NULL,
    stock          INTEGER NOT NULL DEFAULT 0,
    available      BOOLEAN NOT NULL DEFAULT TRUE,
    metadata       STRUCT(publisher VARCHAR, pages INTEGER, language VARCHAR)
);

CREATE TABLE customers (
    id            INTEGER PRIMARY KEY,
    first_name    VARCHAR NOT NULL,
    last_name     VARCHAR NOT NULL,
    email         VARCHAR NOT NULL UNIQUE,
    city          VARCHAR,
    registered_at TIMESTAMP NOT NULL DEFAULT now(),
    preferences   VARCHAR[]
);

CREATE TABLE orders (
    id          INTEGER PRIMARY KEY,
    customer_id INTEGER NOT NULL,   -- → customers.id
    ordered_at  TIMESTAMP NOT NULL,
    status      VARCHAR NOT NULL DEFAULT 'pending',
    total       DECIMAL(10, 2)
);

CREATE TABLE order_items (
    id         INTEGER PRIMARY KEY,
    order_id   INTEGER NOT NULL,   -- → orders.id
    book_id    INTEGER NOT NULL,   -- → books.id
    quantity   INTEGER NOT NULL DEFAULT 1,
    unit_price DECIMAL(8, 2) NOT NULL
);

CREATE TABLE daily_sales (
    sale_date    DATE NOT NULL,
    category_id  INTEGER NOT NULL,   -- → categories.id
    revenue      DECIMAL(12, 2) NOT NULL,
    units_sold   INTEGER NOT NULL,
    PRIMARY KEY (sale_date, category_id)
);

-- ── Categories (6) ───────────────────────────────────────────────────────────

INSERT INTO categories VALUES
    (1, 'Fiction',    'Novels, short stories and literary fiction'),
    (2, 'Science',    'Physics, biology, chemistry and natural sciences'),
    (3, 'History',    'World history, biographies and historical analysis'),
    (4, 'Technology', 'Programming, engineering and digital culture'),
    (5, 'Philosophy', 'Ethics, metaphysics, logic and political thought'),
    (6, 'Children',   'Picture books, middle-grade and young adult');

-- ── Authors (10) — avec champ LIST languages ──────────────────────────────────

INSERT INTO authors VALUES
    (1,  'George',     'Orwell',         'United Kingdom', 1903, ['English']),
    (2,  'Ursula',     'Le Guin',        'United States',  1929, ['English']),
    (3,  'Isaac',      'Asimov',         'United States',  1920, ['English', 'Russian']),
    (4,  'Chimamanda', 'Adichie',        'Nigeria',        1977, ['English', 'Igbo']),
    (5,  'Haruki',     'Murakami',       'Japan',          1949, ['Japanese', 'English']),
    (6,  'Albert',     'Camus',          'France',         1913, ['French', 'Arabic']),
    (7,  'Toni',       'Morrison',       'United States',  1931, ['English']),
    (8,  'Franz',      'Kafka',          'Czech Republic', 1883, ['German', 'Czech']),
    (9,  'Simone',     'de Beauvoir',    'France',         1908, ['French']),
    (10, 'Chinua',     'Achebe',         'Nigeria',        1930, ['English', 'Igbo']);

-- ── Books (20) — avec STRUCT metadata ────────────────────────────────────────

INSERT INTO books VALUES
    (1,  'Nineteen Eighty-Four',         1,  1, 1949, 8.99,  120, TRUE,  {'publisher': 'Secker & Warburg', 'pages': 328, 'language': 'English'}),
    (2,  'Animal Farm',                  1,  1, 1945, 6.99,  85,  TRUE,  {'publisher': 'Secker & Warburg', 'pages': 112, 'language': 'English'}),
    (3,  'The Left Hand of Darkness',    2,  1, 1969, 9.50,  40,  TRUE,  {'publisher': 'Ace Books',         'pages': 286, 'language': 'English'}),
    (4,  'Foundation',                   3,  2, 1951, 11.20, 60,  TRUE,  {'publisher': 'Gnome Press',       'pages': 255, 'language': 'English'}),
    (5,  'I Robot',                      3,  2, 1950, 9.80,  55,  TRUE,  {'publisher': 'Gnome Press',       'pages': 253, 'language': 'English'}),
    (6,  'Purple Hibiscus',              4,  1, 2003, 10.50, 30,  TRUE,  {'publisher': 'Algonquin Books',   'pages': 307, 'language': 'English'}),
    (7,  'Half of a Yellow Sun',         4,  3, 2006, 12.00, 25,  TRUE,  {'publisher': 'Knopf',             'pages': 433, 'language': 'English'}),
    (8,  'Norwegian Wood',               5,  1, 1987, 10.00, 70,  TRUE,  {'publisher': 'Kodansha',          'pages': 296, 'language': 'Japanese'}),
    (9,  'Kafka on the Shore',           5,  1, 2002, 11.50, 50,  TRUE,  {'publisher': 'Shinchosha',        'pages': 505, 'language': 'Japanese'}),
    (10, 'The Stranger',                 6,  5, 1942, 7.90,  95,  TRUE,  {'publisher': 'Gallimard',         'pages': 123, 'language': 'French'}),
    (11, 'The Plague',                   6,  1, 1947, 8.50,  80,  TRUE,  {'publisher': 'Gallimard',         'pages': 308, 'language': 'French'}),
    (12, 'Beloved',                      7,  1, 1987, 11.00, 35,  TRUE,  {'publisher': 'Knopf',             'pages': 324, 'language': 'English'}),
    (13, 'Song of Solomon',              7,  1, 1977, 10.75, 20,  TRUE,  {'publisher': 'Knopf',             'pages': 337, 'language': 'English'}),
    (14, 'The Trial',                    8,  5, 1925, 7.50,  65,  TRUE,  {'publisher': 'Verlag',            'pages': 255, 'language': 'German'}),
    (15, 'The Castle',                   8,  5, 1926, 8.20,  45,  FALSE, {'publisher': 'Verlag',            'pages': 352, 'language': 'German'}),
    (16, 'The Second Sex',               9,  5, 1949, 14.90, 40,  TRUE,  {'publisher': 'Gallimard',         'pages': 832, 'language': 'French'}),
    (17, 'She Came to Stay',             9,  1, 1943, 9.99,  15,  TRUE,  {'publisher': 'Gallimard',         'pages': 404, 'language': 'French'}),
    (18, 'Things Fall Apart',            10, 1, 1958, 8.75,  90,  TRUE,  {'publisher': 'Heinemann',         'pages': 209, 'language': 'English'}),
    (19, 'Arrow of God',                 10, 3, 1964, 9.25,  30,  TRUE,  {'publisher': 'Heinemann',         'pages': 230, 'language': 'English'}),
    (20, 'No Longer at Ease',            10, 1, 1960, 8.00,  40,  TRUE,  {'publisher': 'Heinemann',         'pages': 170, 'language': 'English'});

-- ── Customers (12) — avec champ LIST preferences ─────────────────────────────

INSERT INTO customers VALUES
    (1,  'Alice',   'Smith',    'alice.smith@example.com',   'Paris',       '2024-01-15 09:00:00', ['Fiction', 'Philosophy']),
    (2,  'Bob',     'Jones',    'bob.jones@example.com',     'London',      '2024-02-20 14:30:00', ['Science', 'Technology']),
    (3,  'Clara',   'Martin',   'clara.martin@example.com',  'Berlin',      '2024-03-05 11:00:00', ['Fiction', 'History']),
    (4,  'David',   'Brown',    'david.brown@example.com',   'Tokyo',       '2024-04-12 08:45:00', ['Fiction']),
    (5,  'Eva',     'Wilson',   'eva.wilson@example.com',    'New York',    '2024-05-18 16:20:00', ['Philosophy', 'Fiction']),
    (6,  'Frank',   'Taylor',   'frank.taylor@example.com',  'Sydney',      '2024-06-01 10:10:00', ['Technology', 'Science']),
    (7,  'Grace',   'Anderson', 'grace.a@example.com',       'Toronto',     '2024-07-22 13:00:00', ['History', 'Fiction']),
    (8,  'Henry',   'Thomas',   'henry.t@example.com',       'Lagos',       '2024-08-09 17:30:00', ['Fiction', 'Children']),
    (9,  'Irene',   'Jackson',  'irene.j@example.com',       'Buenos Aires','2024-09-14 09:15:00', ['Fiction']),
    (10, 'Jack',    'White',    'jack.white@example.com',    'Seoul',       '2024-10-03 11:45:00', ['Science', 'Technology']),
    (11, 'Karla',   'Moore',    'karla.m@example.com',       'Paris',       '2024-11-17 15:00:00', ['Philosophy']),
    (12, 'Liam',    'Harris',   'liam.harris@example.com',   'London',      '2024-12-05 12:30:00', ['Fiction', 'History', 'Science']);

-- ── Orders (20) ──────────────────────────────────────────────────────────────

INSERT INTO orders (id, customer_id, ordered_at, status) VALUES
    (1,  3,  '2025-01-10 10:30:00', 'delivered'),
    (2,  1,  '2025-01-25 14:22:00', 'delivered'),
    (3,  5,  '2025-02-03 11:05:00', 'delivered'),
    (4,  2,  '2025-02-18 09:15:00', 'delivered'),
    (5,  8,  '2025-03-02 16:40:00', 'shipped'),
    (6,  4,  '2025-03-15 18:10:00', 'delivered'),
    (7,  1,  '2025-03-28 13:24:00', 'delivered'),
    (8,  7,  '2025-04-06 11:55:00', 'delivered'),
    (9,  9,  '2025-04-20 08:30:00', 'delivered'),
    (10, 6,  '2025-05-01 15:00:00', 'shipped'),
    (11, 11, '2025-05-14 10:00:00', 'delivered'),
    (12, 3,  '2025-05-25 14:10:00', 'delivered'),
    (13, 12, '2025-06-02 09:45:00', 'pending'),
    (14, 2,  '2025-06-10 16:30:00', 'delivered'),
    (15, 5,  '2025-06-18 11:20:00', 'delivered'),
    (16, 10, '2025-07-01 13:00:00', 'shipped'),
    (17, 4,  '2025-07-15 17:45:00', 'delivered'),
    (18, 7,  '2025-08-01 08:00:00', 'delivered'),
    (19, 1,  '2025-08-20 10:30:00', 'pending'),
    (20, 8,  '2025-09-05 14:00:00', 'shipped');

-- ── Order items ───────────────────────────────────────────────────────────────

INSERT INTO order_items (id, order_id, book_id, quantity, unit_price) VALUES
    (1,  1,  3,  1,  9.50),
    (2,  1,  10, 2,  7.90),
    (3,  2,  1,  1,  8.99),
    (4,  2,  16, 1,  14.90),
    (5,  3,  10, 1,  7.90),
    (6,  3,  11, 1,  8.50),
    (7,  4,  4,  2,  11.20),
    (8,  4,  5,  1,  9.80),
    (9,  5,  18, 1,  8.75),
    (10, 6,  8,  1,  10.00),
    (11, 7,  2,  3,  6.99),
    (12, 7,  14, 1,  7.50),
    (13, 8,  7,  1,  12.00),
    (14, 9,  12, 1,  11.00),
    (15, 10, 6,  2,  10.50),
    (16, 11, 16, 1,  14.90),
    (17, 12, 9,  1,  11.50),
    (18, 12, 18, 2,  8.75),
    (19, 13, 4,  1,  11.20),
    (20, 14, 1,  2,  8.99),
    (21, 14, 2,  1,  6.99),
    (22, 15, 10, 1,  7.90),
    (23, 15, 11, 1,  8.50),
    (24, 16, 5,  2,  9.80),
    (25, 17, 8,  1,  10.00),
    (26, 17, 9,  1,  11.50),
    (27, 18, 18, 3,  8.75),
    (28, 19, 16, 1,  14.90),
    (29, 20, 3,  2,  9.50),
    (30, 20, 6,  1,  10.50);

-- ── Mise à jour des totaux commande ──────────────────────────────────────────

UPDATE orders
SET total = (
    SELECT SUM(quantity * unit_price)
    FROM order_items
    WHERE order_id = orders.id
);

-- ── Table analytique daily_sales (90 jours × 6 catégories) ───────────────────

INSERT INTO daily_sales
SELECT
    (DATE '2025-01-01' + INTERVAL (n) DAY)::DATE AS sale_date,
    cat_id,
    ROUND((50 + (n * cat_id * 7) % 400 + (n % 13) * 12)::DECIMAL, 2) AS revenue,
    3 + (n + cat_id) % 20 AS units_sold
FROM generate_series(0, 89) AS t(n)
CROSS JOIN generate_series(1, 6) AS c(cat_id);

-- ── Résumé ────────────────────────────────────────────────────────────────────

SELECT 'categories'  AS table_name, COUNT(*) AS rows FROM categories  UNION ALL
SELECT 'authors',    COUNT(*) FROM authors    UNION ALL
SELECT 'books',      COUNT(*) FROM books      UNION ALL
SELECT 'customers',  COUNT(*) FROM customers  UNION ALL
SELECT 'orders',     COUNT(*) FROM orders     UNION ALL
SELECT 'order_items',COUNT(*) FROM order_items UNION ALL
SELECT 'daily_sales',COUNT(*) FROM daily_sales;

-- =============================================================================
-- EXEMPLES DE REQUÊTES À TESTER DANS L'ÉDITEUR SQL ROWDY
-- =============================================================================

-- ── Types spéciaux DuckDB ─────────────────────────────────────────────────────

-- Lire le champ STRUCT (clic sur badge [obj] dans Rowdy)
-- SELECT title, metadata, metadata.publisher, metadata.pages FROM books LIMIT 5;

-- Lire le champ LIST (clic sur badge [arr:N] dans Rowdy)
-- SELECT first_name, last_name, languages FROM authors;
-- SELECT first_name, preferences FROM customers;

-- Unnest d'une liste (1 ligne par élément)
-- SELECT first_name, unnest(languages) AS language FROM authors ORDER BY first_name;

-- Filtrer par contenu de liste
-- SELECT first_name, last_name FROM authors WHERE list_contains(languages, 'French');

-- ── Analytique (window functions) ────────────────────────────────────────────

-- Rang des livres par prix dans chaque catégorie
-- SELECT title, price,
--        RANK() OVER (PARTITION BY category_id ORDER BY price DESC) AS price_rank
-- FROM books ORDER BY category_id, price_rank;

-- Cumul du chiffre d'affaires par catégorie sur la période
-- SELECT sale_date, category_id, revenue,
--        SUM(revenue) OVER (PARTITION BY category_id ORDER BY sale_date) AS running_total
-- FROM daily_sales WHERE category_id = 1 ORDER BY sale_date LIMIT 20;

-- Top 3 catégories par revenu total
-- SELECT c.name, SUM(ds.revenue) AS total_revenue
-- FROM daily_sales ds JOIN categories c ON ds.category_id = c.id
-- GROUP BY c.name ORDER BY total_revenue DESC LIMIT 3;

-- ── Parquet / CSV (si fichiers disponibles) ───────────────────────────────────

-- DuckDB peut requêter directement des fichiers sans les importer :
-- SELECT * FROM read_csv('/path/to/data.csv', header=true) LIMIT 10;
-- SELECT * FROM read_parquet('/path/to/data.parquet') LIMIT 10;
-- SELECT COUNT(*) FROM read_parquet('/path/to/*.parquet');
-- COPY (SELECT * FROM books) TO '/tmp/books.parquet' (FORMAT PARQUET);
-- COPY (SELECT * FROM daily_sales) TO '/tmp/sales.csv' (FORMAT CSV, HEADER);
