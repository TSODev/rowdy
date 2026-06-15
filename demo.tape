# Rowdy demo — generate with: vhs demo.tape
# Requires: vhs (https://github.com/charmbracelet/vhs)
# A real SQLite database must exist at /tmp/demo.db (see below)
#
# Setup: sqlite3 /tmp/demo.db "
#   CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT);
#   CREATE TABLE books (id INTEGER PRIMARY KEY, title TEXT, author_id INTEGER REFERENCES authors(id), year INTEGER);
#   INSERT INTO authors VALUES (1,'Tolkien'),(2,'Herbert'),(3,'Le Guin');
#   INSERT INTO books VALUES
#     (1,'The Lord of the Rings',1,1954),
#     (2,'Dune',2,1965),
#     (3,'The Left Hand of Darkness',3,1969),
#     (4,'The Fellowship of the Ring',1,1954),
#     (5,'Children of Dune',2,1976);
# "

Output assets/demo.gif

Set Shell "bash"
Set FontSize 13
Set Width 1100
Set Height 650
Set Theme "Catppuccin Mocha"
Set Framerate 24
Set PlaybackSpeed 1.0
Set TypingSpeed 80ms

# Launch rowdy
Type "cargo run --quiet"
Enter
Sleep 3s

# Connection screen — type SQLite URL
Type "n"
Sleep 500ms
Type "Tab"
Sleep 300ms
# cycle to sqlite
Type "Tab"
Sleep 300ms
Type "sqlite:///tmp/demo.db"
Sleep 500ms
Enter
Sleep 2s

# Table list — navigate to books
Type "j"
Sleep 400ms
Enter
Sleep 1500ms

# Data Grid — navigate rows
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms

# Resize a column
Type "]"
Sleep 300ms
Type "]"
Sleep 300ms

# Open FK sub-grid on author_id
Type "l"
Sleep 400ms
Type "l"
Sleep 400ms
Enter
Sleep 1500ms

# Back to data grid
Escape
Sleep 800ms

# Filter
Type "f"
Sleep 500ms
Type "Dune"
Enter
Sleep 1000ms

# Clear filter
Type "F"
Sleep 800ms

# Export
Type "E"
Sleep 800ms
Type "c"
Sleep 1500ms

# Open SQL editor
Type "q"
Sleep 500ms
Type "e"
Sleep 800ms

# Type and run a query
Type "SELECT title, year FROM books ORDER BY year"
Sleep 500ms
Ctrl+Enter
Sleep 1500ms

# Navigate results
Type "j"
Sleep 300ms
Type "j"
Sleep 300ms

# Quit
Ctrl+c
Sleep 500ms
