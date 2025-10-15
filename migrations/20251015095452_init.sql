CREATE TABLE material (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    long_name TEXT NOT NULL,
    description TEXT
);

CREATE TABLE item (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    price REAL NOT NULL,
    amount INTEGER NOT NULL,
    active BOOLEAN NOT NULL,
    due_date TIMESTAMPTZ NOT NULL,
    material_id UUID REFERENCES material(id)
);

