import { writeFile } from "node:fs/promises";
import { categories, products } from "../../frontend/src/data/products.js";

const q = (value) => `'${String(value ?? "").replaceAll("'", "''")}'`;
const sqlJson = (value) => `${q(JSON.stringify(value))}::jsonb`;
const slug = (value) => value.toLowerCase().trim().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

const statements = [
  "-- Existing storefront catalog, imported as initial catalog data.",
  ...categories.map((c, i) => `INSERT INTO categories(name,slug,position) VALUES(${q(c.name)},${q(slug(c.name))},${i}) ON CONFLICT(slug) DO NOTHING;`),
];

for (const p of products) {
  statements.push(`INSERT INTO products(name,slug,category_id,subcategory,description,details,tag,price,colors,sizes,is_new,best,status) SELECT ${q(p.name)},${q(p.id)},c.id,${q(p.subcategory)},${q(p.description)},${q(p.details)},${q(p.tag)},${Number(p.price)},${sqlJson(p.colors)},${sqlJson(p.sizes)},${Boolean(p.isNew)},${Boolean(p.best)},'published' FROM categories c WHERE c.slug=${q(slug(p.category))} ON CONFLICT(slug) DO NOTHING;`);
  for (const [position, image] of p.images.entries()) {
    statements.push(`INSERT INTO product_images(product_id,url,source,alt,position) SELECT id,${q(image)},'external',${q(`${p.name} — view ${position + 1}`)},${position} FROM products WHERE slug=${q(p.id)} ON CONFLICT DO NOTHING;`);
  }
}

await writeFile(new URL("../migrations/0002_catalog_seed.sql", import.meta.url), `${statements.join("\n")}\n`, "utf8");
console.log(`Generated seed migration for ${products.length} products and ${categories.length} categories.`);
