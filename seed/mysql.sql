-- =============================================================================
-- Rowdy test seed — MySQL / MariaDB
-- Usage : mysql -u <user> -p <dbname> < seed/mysql.sql
-- Replay: same command — drops and recreates everything cleanly
-- Requires MySQL >= 8.0 or MariaDB >= 10.2 (recursive CTEs)
-- =============================================================================

SET FOREIGN_KEY_CHECKS = 0;
SET @@SESSION.cte_max_recursion_depth = 500;

-- ── Drop (reverse dependency order) ──────────────────────────────────────────
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS customers;
DROP TABLE IF EXISTS books;
DROP TABLE IF EXISTS authors;
DROP TABLE IF EXISTS categories;

SET FOREIGN_KEY_CHECKS = 1;

-- ── Schema ────────────────────────────────────────────────────────────────────

CREATE TABLE categories (
    id          INT AUTO_INCREMENT PRIMARY KEY,
    name        VARCHAR(100) NOT NULL,
    description TEXT
);

CREATE TABLE authors (
    id          INT AUTO_INCREMENT PRIMARY KEY,
    first_name  VARCHAR(100) NOT NULL,
    last_name   VARCHAR(100) NOT NULL,
    country     VARCHAR(100) NOT NULL,
    birth_year  INT
);

CREATE TABLE books (
    id             INT AUTO_INCREMENT PRIMARY KEY,
    title          VARCHAR(255)   NOT NULL,
    author_id      INT            NOT NULL,
    category_id    INT            NOT NULL,
    published_year INT,
    price          DECIMAL(8,2)   NOT NULL,
    stock          INT            NOT NULL DEFAULT 0,
    available      TINYINT(1)     NOT NULL DEFAULT 1,
    CONSTRAINT fk_book_author   FOREIGN KEY (author_id)   REFERENCES authors(id),
    CONSTRAINT fk_book_category FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE TABLE customers (
    id            INT AUTO_INCREMENT PRIMARY KEY,
    first_name    VARCHAR(100) NOT NULL,
    last_name     VARCHAR(100) NOT NULL,
    email         VARCHAR(255) NOT NULL UNIQUE,
    city          VARCHAR(100),
    registered_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE orders (
    id          INT AUTO_INCREMENT PRIMARY KEY,
    customer_id INT          NOT NULL,
    ordered_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status      VARCHAR(20)  NOT NULL DEFAULT 'pending',
    total       DECIMAL(10,2),
    CONSTRAINT fk_order_customer FOREIGN KEY (customer_id) REFERENCES customers(id)
);

CREATE TABLE order_items (
    id         INT AUTO_INCREMENT PRIMARY KEY,
    order_id   INT          NOT NULL,
    book_id    INT          NOT NULL,
    quantity   INT          NOT NULL DEFAULT 1,
    unit_price DECIMAL(8,2) NOT NULL,
    CONSTRAINT fk_item_order FOREIGN KEY (order_id) REFERENCES orders(id),
    CONSTRAINT fk_item_book  FOREIGN KEY (book_id)  REFERENCES books(id)
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


INSERT INTO books (title, author_id, category_id, published_year, price, stock, available)
WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 300
),
adj(n, word) AS (
    SELECT 0, 'The Great' UNION ALL SELECT 1, 'Lost'    UNION ALL
    SELECT 2, 'Dark'      UNION ALL SELECT 3, 'Hidden'  UNION ALL
    SELECT 4, 'Eternal'   UNION ALL SELECT 5, 'Sacred'  UNION ALL
    SELECT 6, 'Ancient'   UNION ALL SELECT 7, 'Modern'  UNION ALL
    SELECT 8, 'Silent'    UNION ALL SELECT 9, 'Golden'
),
noun(n, word) AS (
    SELECT 0, 'Journey'     UNION ALL SELECT 1, 'Mystery'    UNION ALL
    SELECT 2, 'Empire'      UNION ALL SELECT 3, 'Legacy'     UNION ALL
    SELECT 4, 'Chronicles'  UNION ALL SELECT 5, 'Saga'       UNION ALL
    SELECT 6, 'Quest'       UNION ALL SELECT 7, 'Secret'     UNION ALL
    SELECT 8, 'Voyage'      UNION ALL SELECT 9, 'Dream'
)
SELECT
    CONCAT(adj.word, ' ', noun.word, ' — vol. ', seq.n),
    (seq.n % 20) + 1,
    (seq.n % 6)  + 1,
    1950 + (seq.n % 74),
    ROUND(9.99 + (seq.n % 50) * 0.70, 2),
    (seq.n * 3) % 40,
    IF(seq.n % 8 = 0, 0, 1)
FROM seq
JOIN adj  ON adj.n  = seq.n % 10
JOIN noun ON noun.n = seq.n % 10;

-- ── Customers (50) ───────────────────────────────────────────────────────────

INSERT INTO customers (first_name, last_name, email, city, registered_at)
WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 50
),
fn(n, word) AS (
    SELECT 0,'Alice' UNION ALL SELECT 1,'Bob'   UNION ALL SELECT 2,'Clara'  UNION ALL
    SELECT 3,'David' UNION ALL SELECT 4,'Eva'   UNION ALL SELECT 5,'Frank'  UNION ALL
    SELECT 6,'Grace' UNION ALL SELECT 7,'Henry' UNION ALL SELECT 8,'Irene'  UNION ALL
    SELECT 9,'Jack'
),
ln(n, word) AS (
    SELECT 0,'Smith'    UNION ALL SELECT 1,'Jones'    UNION ALL SELECT 2,'Martin'   UNION ALL
    SELECT 3,'Brown'    UNION ALL SELECT 4,'Wilson'   UNION ALL SELECT 5,'Taylor'   UNION ALL
    SELECT 6,'Anderson' UNION ALL SELECT 7,'Thomas'   UNION ALL SELECT 8,'Jackson'  UNION ALL
    SELECT 9,'White'
),
ct(n, word) AS (
    SELECT 0,'Paris'    UNION ALL SELECT 1,'London'   UNION ALL SELECT 2,'Berlin'      UNION ALL
    SELECT 3,'Tokyo'    UNION ALL SELECT 4,'New York' UNION ALL SELECT 5,'Sydney'      UNION ALL
    SELECT 6,'Toronto'  UNION ALL SELECT 7,'Lagos'    UNION ALL SELECT 8,'Buenos Aires' UNION ALL
    SELECT 9,'Seoul'
)
SELECT
    fn.word,
    ln.word,
    CONCAT(LOWER(fn.word), '.', LOWER(ln.word), seq.n, '@example.com'),
    ct.word,
    DATE_SUB(NOW(), INTERVAL seq.n * 7 DAY)
FROM seq
JOIN fn ON fn.n = seq.n % 10
JOIN ln ON ln.n = seq.n % 10
JOIN ct ON ct.n = seq.n % 10;

-- ── Orders (300 — 6 per customer) ────────────────────────────────────────────

INSERT INTO orders (customer_id, ordered_at, status)
WITH RECURSIVE
seq(n) AS (
    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 300
),
st(n, word) AS (
    SELECT 0,'pending'   UNION ALL SELECT 1,'shipped'   UNION ALL
    SELECT 2,'delivered' UNION ALL SELECT 3,'cancelled' UNION ALL
    SELECT 4,'pending'   UNION ALL SELECT 5,'shipped'
)

SELECT
    ((seq.n - 1) % 50) + 1,
    DATE_SUB(NOW(), INTERVAL seq.n * 2 DAY),
    st.word
FROM seq
JOIN st ON st.n = seq.n % 6;

-- ── Order items (2 per order → 600 rows) ─────────────────────────────────────

INSERT INTO order_items (order_id, book_id, quantity, unit_price)
SELECT
    o.id,
    ((o.id + 0) % 300) + 1,
    (o.id % 3) + 1,
    b.price
FROM orders o
JOIN books b ON b.id = ((o.id + 0) % 300) + 1;

INSERT INTO order_items (order_id, book_id, quantity, unit_price)
SELECT
    o.id,
    ((o.id + 1) % 300) + 1,
    (o.id % 3) + 1,
    b.price
FROM orders o
JOIN books b ON b.id = ((o.id + 1) % 300) + 1;

-- ── Update order totals ───────────────────────────────────────────────────────

UPDATE orders o
JOIN (
    SELECT order_id, SUM(quantity * unit_price) AS t
    FROM order_items
    GROUP BY order_id
) s ON s.order_id = o.id
SET o.total = s.t;

-- ── Summary ───────────────────────────────────────────────────────────────────

SELECT 'categories'  AS `table`, COUNT(*) AS cw FROM categories
UNION ALL SELECT 'authors',     COUNT(*) AS cw FROM authors
UNION ALL SELECT 'books',       COUNT(*) AS cw FROM books
UNION ALL SELECT 'customers',   COUNT(*) AS cw FROM customers
UNION ALL SELECT 'orders',      COUNT(*) AS cw FROM orders
UNION ALL SELECT 'order_items', COUNT(*) AS cw FROM order_items;
