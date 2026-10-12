CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE categories (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), name text NOT NULL UNIQUE,
  slug text NOT NULL UNIQUE, active boolean NOT NULL DEFAULT true,
  position integer NOT NULL DEFAULT 0, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE products (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), name text NOT NULL, slug text NOT NULL UNIQUE,
  sku text UNIQUE, category_id uuid NOT NULL REFERENCES categories(id) ON DELETE RESTRICT,
  subcategory text, description text NOT NULL DEFAULT '', details text NOT NULL DEFAULT '',
  tag text, price numeric(12,2) NOT NULL CHECK(price >= 0), compare_at_price numeric(12,2) CHECK(compare_at_price IS NULL OR compare_at_price >= price),
  colors jsonb NOT NULL DEFAULT '[]', sizes jsonb NOT NULL DEFAULT '[]',
  is_new boolean NOT NULL DEFAULT false, best boolean NOT NULL DEFAULT false,
  stock integer NOT NULL DEFAULT 0 CHECK(stock >= 0), status text NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','published','archived')),
  created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX products_catalog_idx ON products(status, category_id, created_at DESC);
CREATE INDEX products_price_idx ON products(price);
CREATE TABLE product_images (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(), product_id uuid NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  url text NOT NULL CHECK(url LIKE 'https://%'), public_id text, source text NOT NULL CHECK(source IN ('cloudinary','external')),
  alt text NOT NULL DEFAULT '', position integer NOT NULL DEFAULT 0, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX product_images_product_idx ON product_images(product_id,position);

CREATE TABLE users (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), full_name text NOT NULL, email text NOT NULL UNIQUE,
 password_hash text, role text NOT NULL DEFAULT 'customer' CHECK(role IN ('customer','admin')),
 status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','active','blocked','disabled')),
 email_verified_at timestamptz, google_subject text UNIQUE, created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE email_verification_codes (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 code_hash text NOT NULL, expires_at timestamptz NOT NULL, attempts smallint NOT NULL DEFAULT 0,
 consumed_at timestamptz, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX verification_user_idx ON email_verification_codes(user_id,created_at DESC);
CREATE TABLE user_sessions (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 token_hash text NOT NULL UNIQUE, expires_at timestamptz NOT NULL, revoked_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE addresses (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 recipient text NOT NULL, phone text NOT NULL DEFAULT '', street text NOT NULL, city text NOT NULL,
 postal_code text NOT NULL DEFAULT '', country text NOT NULL, instructions text NOT NULL DEFAULT '', created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE carts (user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE, updated_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE cart_items (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
 product_id uuid NOT NULL REFERENCES products(id) ON DELETE RESTRICT, size text NOT NULL DEFAULT '', color text NOT NULL DEFAULT '',
 quantity integer NOT NULL CHECK(quantity BETWEEN 1 AND 20), UNIQUE(user_id,product_id,size,color)
);
CREATE TABLE orders (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), order_number text NOT NULL UNIQUE,
 user_id uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
 status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','confirmed','processing','shipped','delivered','cancelled','refunded')),
 payment_method text NOT NULL DEFAULT 'cash_on_delivery', payment_status text NOT NULL DEFAULT 'unpaid' CHECK(payment_status IN ('unpaid','paid','refunded')),
 shipping_snapshot jsonb NOT NULL, subtotal numeric(12,2) NOT NULL CHECK(subtotal >= 0), shipping numeric(12,2) NOT NULL CHECK(shipping >= 0), total numeric(12,2) NOT NULL CHECK(total >= 0),
 idempotency_key text, created_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(user_id,idempotency_key)
);
CREATE TABLE order_items (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), order_id uuid NOT NULL REFERENCES orders(id) ON DELETE RESTRICT,
 product_id uuid REFERENCES products(id) ON DELETE SET NULL, product_name text NOT NULL, sku text, variant jsonb NOT NULL DEFAULT '{}',
 quantity integer NOT NULL CHECK(quantity > 0), unit_price numeric(12,2) NOT NULL CHECK(unit_price >= 0)
);
CREATE TABLE ip_block_rules (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), network cidr NOT NULL UNIQUE, reason text NOT NULL,
 created_by uuid NOT NULL REFERENCES users(id) ON DELETE RESTRICT, created_at timestamptz NOT NULL DEFAULT now(), revoked_at timestamptz
);
CREATE TABLE audit_logs (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), actor_id uuid REFERENCES users(id) ON DELETE SET NULL,
 action text NOT NULL, target_type text NOT NULL, target_id text, metadata jsonb NOT NULL DEFAULT '{}', created_at timestamptz NOT NULL DEFAULT now()
);
