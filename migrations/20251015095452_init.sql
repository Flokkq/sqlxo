CREATE TABLE supplier (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE material (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    long_name TEXT NOT NULL,
    description TEXT,
    supplier_id UUID REFERENCES supplier(id)
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

CREATE TABLE tag (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE item_tag (
    id UUID PRIMARY KEY,
    item_id UUID NOT NULL REFERENCES item(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    note TEXT,
    UNIQUE (item_id, tag_id)
);

CREATE TABLE app_user (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE profile (
    id UUID PRIMARY KEY,
    user_id UUID UNIQUE NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
    bio TEXT
);

CREATE TABLE hard_delete_item (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    price REAL NOT NULL,
    updated_at TIMESTAMPTZ
);

CREATE TABLE soft_delete_item (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    price REAL NOT NULL,
    deleted_at TIMESTAMPTZ
);

CREATE TABLE update_item (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    price REAL NOT NULL,
    ignored_field TEXT NOT NULL,
    updated_at TIMESTAMPTZ
);

CREATE TABLE create_item (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    price REAL NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
