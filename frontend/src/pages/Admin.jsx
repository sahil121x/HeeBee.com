import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api";
import { money } from "../data/products";

const blankProduct = {
  name: "", slug: "", sku: "", category: "", subcategory: "", price: "", compareAt: "",
  description: "", details: "", tag: "", stock: "0", status: "draft", isNew: false,
  best: false, colorsText: "", sizesText: "", imagesText: "",
};
const splitLines = (value) => value.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);
const transitions = {
  pending: ["confirmed", "cancelled"], confirmed: ["processing", "cancelled"],
  processing: ["shipped", "cancelled"], shipped: ["delivered"], delivered: ["refunded"],
  cancelled: [], refunded: [],
};

export default function Admin({ user, loading: authLoading }) {
  const [tab, setTab] = useState("overview");
  const [stats, setStats] = useState(null);
  const [products, setProducts] = useState([]);
  const [categories, setCategories] = useState([]);
  const [orders, setOrders] = useState([]);
  const [customers, setCustomers] = useState([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [editing, setEditing] = useState(null);
  const [editorOpen, setEditorOpen] = useState(false);
  const [form, setForm] = useState(blankProduct);
  const [categoryName, setCategoryName] = useState("");
  const [search, setSearch] = useState("");

  const loadProducts = useCallback(async () => {
    const [items, cats] = await Promise.all([api(`/admin/products?search=${encodeURIComponent(search)}`), api("/admin/categories")]);
    setProducts(items); setCategories(cats);
  }, [search]);
  const loadTab = useCallback(async () => {
    if (!user || user.role !== "admin") return;
    setError("");
    try {
      if (tab === "overview") {
        const data = await api("/admin/dashboard"); setStats(data);
      } else if (tab === "products") await loadProducts();
      else if (tab === "orders") setOrders(await api("/admin/orders"));
      else if (tab === "customers") setCustomers(await api("/admin/users"));
    } catch (err) { setError(err.message || "Could not load administrator data."); }
  }, [user, tab, loadProducts]);
  useEffect(() => { const task = window.setTimeout(() => { loadTab(); }, 0); return () => window.clearTimeout(task); }, [loadTab]);

  const saveProduct = async (event) => {
    event.preventDefault(); setBusy(true); setError(""); setNotice("");
    try {
      const colors = splitLines(form.colorsText).map((line) => {
        const [name, hex] = line.split("|").map((part) => part.trim());
        if (!name || !/^#[\da-f]{6}$/i.test(hex || "")) throw new Error("Enter colors as Name|#RRGGBB, one per line.");
        return [name, hex];
      });
      const payload = {
        name: form.name, slug: form.slug || null, sku: form.sku || null, category: form.category,
        subcategory: form.subcategory || null, price: Number(form.price),
        compare_at_price: form.compareAt ? Number(form.compareAt) : null,
        description: form.description, details: form.details || "", tag: form.tag || null,
        stock: Number(form.stock), status: form.status, is_new: form.isNew, best: form.best,
        colors, sizes: splitLines(form.sizesText.replaceAll(",", "\n")), images: splitLines(form.imagesText),
      };
      await api(editing ? `/admin/products/${editing}` : "/admin/products", {
        method: editing ? "PATCH" : "POST", body: JSON.stringify(payload),
      });
      setEditing(null); setEditorOpen(false); setForm(blankProduct); setNotice(editing ? "Product updated." : "Product created."); await loadProducts();
    } catch (err) { setError(err.message || "Could not save product."); }
    finally { setBusy(false); }
  };
  const startEdit = (p) => {
    setEditing(p.id); setEditorOpen(true); setForm({ ...blankProduct, ...p, isNew: p.isNew ?? p.is_new ?? false, compareAt: p.compare_at_price ?? "",
      colorsText: (p.colors || []).map(([name, color]) => `${name}|${color}`).join("\n"),
      sizesText: (p.sizes || []).join(", "), imagesText: (p.images || []).join("\n"),
    }); setError(""); setNotice("");
  };
  const archiveProduct = async (p) => {
    setBusy(true); setError(""); setNotice("");
    try { await api(`/admin/products/${p.id}`, { method: "DELETE" }); setNotice(`${p.name} archived.`); await loadProducts(); }
    catch (err) { setError(err.message); } finally { setBusy(false); }
  };
  const updateOrder = async (order, status) => {
    setBusy(true); setError("");
    try { await api(`/admin/orders/${order.id}`, { method: "PATCH", body: JSON.stringify({ status }) }); setOrders((rows) => rows.map((x) => x.id === order.id ? { ...x, status, payment_status: status === "refunded" ? "refunded" : x.payment_status } : x)); }
    catch (err) { setError(err.message); } finally { setBusy(false); }
  };
  const updateCustomer = async (customer, status) => {
    setBusy(true); setError("");
    try { await api(`/admin/users/${customer.id}`, { method: "PATCH", body: JSON.stringify({ status }) }); setCustomers((rows) => rows.map((x) => x.id === customer.id ? { ...x, status } : x)); }
    catch (err) { setError(err.message); } finally { setBusy(false); }
  };
  const createCategory = async (event) => {
    event.preventDefault(); setBusy(true); setError("");
    try { await api("/admin/categories", { method: "POST", body: JSON.stringify({ name: categoryName }) }); setCategoryName(""); setCategories(await api("/admin/categories")); setNotice("Category created."); }
    catch (err) { setError(err.message); } finally { setBusy(false); }
  };
  const setCategoryActive = async (category, active) => {
    setBusy(true); setError(""); setNotice("");
    try {
      await api(`/admin/categories/${encodeURIComponent(category.slug)}`, {
        method: "PATCH", body: JSON.stringify({ active }),
      });
      setCategories(await api("/admin/categories"));
      setNotice(active ? `${category.name} restored.` : `${category.name} removed from the storefront.`);
    } catch (err) { setError(err.message); } finally { setBusy(false); }
  };

  if (authLoading) return <main className="main-content admin-page"><p>Checking administrator access…</p></main>;
  if (!user || user.role !== "admin") return <main className="main-content admin-page"><div className="eyebrow">HEEBEE STUDIO</div><h1>Administrator access required<span className="orange-period">.</span></h1><p>Sign in with an administrator account to view this page.</p><Link className="button button-dark" to="/">Back to the store</Link></main>;

  const tabs = [["overview", "Overview"], ["products", "Products"], ["orders", "Orders"], ["customers", "Customers"]];
  return <main className="main-content admin-page">
    <div className="eyebrow"><span className="eyebrow-dot"/> HEEBEE STUDIO / ADMIN</div>
    <div className="admin-page-heading"><div><h1>Store dashboard<span className="orange-period">.</span></h1><p>Signed in as {user.email}</p></div><Link className="button button-outline" to="/">View storefront ↗</Link></div>
    <nav className="admin-tabs" aria-label="Administrator sections">{tabs.map(([value, label]) => <button key={value} className={tab === value ? "active" : ""} onClick={() => { setTab(value); setNotice(""); setError(""); }}>{label}</button>)}</nav>
    {error && <p className="admin-feedback error" role="alert">{error}</p>}{notice && <p className="admin-feedback" role="status">{notice}</p>}

    {tab === "overview" && <section className="admin-stats">{[["Published products", stats?.products], ["All orders", stats?.orders], ["Customers", stats?.customers], ["Pending orders", stats?.pending_orders]].map(([label, value]) => <article className="admin-stat" key={label}><span>{label}</span><strong>{value ?? "—"}</strong></article>)}</section>}

    {tab === "products" && <section className="admin-workspace">
      <div className="admin-section-heading"><div><h2>Products</h2><p>Add and update the pieces shown in the storefront.</p></div><button className="button button-dark" onClick={() => { setEditing(null); setEditorOpen(true); setForm({ ...blankProduct, category: categories[0]?.name || "" }); }}>Add product</button></div>
      {editorOpen && <form className="admin-product-form" onSubmit={saveProduct}>
        <h3>{editing ? "Edit product" : "New product"}</h3><div className="admin-form-grid">
          <label>Product name<input required maxLength={180} value={form.name} onChange={e=>setForm({...form,name:e.target.value})}/></label><label>URL slug<input value={form.slug||""} onChange={e=>setForm({...form,slug:e.target.value})} placeholder="Generated from name"/></label>
          <label>SKU<input value={form.sku||""} onChange={e=>setForm({...form,sku:e.target.value})}/></label><label>Category<select required value={form.category||""} onChange={e=>setForm({...form,category:e.target.value})}><option value="">Choose category</option>{categories.filter(c=>c.active).map(c=><option key={c.slug}>{c.name}</option>)}</select></label>
          <label>Subcategory<input value={form.subcategory||""} onChange={e=>setForm({...form,subcategory:e.target.value})}/></label><label>Price<input required type="number" min="0" step="0.01" value={form.price} onChange={e=>setForm({...form,price:e.target.value})}/></label>
          <label>Compare-at price<input type="number" min="0" step="0.01" value={form.compareAt} onChange={e=>setForm({...form,compareAt:e.target.value})}/></label><label>Inventory<input required type="number" min="0" step="1" value={form.stock} onChange={e=>setForm({...form,stock:e.target.value})}/></label>
          <label>Status<select value={form.status} onChange={e=>setForm({...form,status:e.target.value})}><option value="draft">Draft</option><option value="published">Published</option><option value="archived">Archived</option></select></label><label>Badge / tag<input value={form.tag||""} onChange={e=>setForm({...form,tag:e.target.value})}/></label>
          <label className="admin-wide">Description<textarea rows="3" value={form.description} onChange={e=>setForm({...form,description:e.target.value})}/></label><label className="admin-wide">Details and fit<textarea rows="2" value={form.details||""} onChange={e=>setForm({...form,details:e.target.value})}/></label>
          <label>Colors (Name|#RRGGBB, one per line)<textarea rows="4" value={form.colorsText} onChange={e=>setForm({...form,colorsText:e.target.value})} placeholder={'Washed Black|#292A27\nChalk|#E8E5DC'}/></label><label>Sizes (comma separated)<textarea rows="4" value={form.sizesText} onChange={e=>setForm({...form,sizesText:e.target.value})} placeholder="S, M, L, XL"/></label>
          <label className="admin-wide">Image URLs (HTTPS, one per line)<textarea rows="3" value={form.imagesText} onChange={e=>setForm({...form,imagesText:e.target.value})} placeholder="https://…"/></label>
          <label className="admin-check"><input type="checkbox" checked={form.isNew} onChange={e=>setForm({...form,isNew:e.target.checked})}/> New arrival</label><label className="admin-check"><input type="checkbox" checked={form.best} onChange={e=>setForm({...form,best:e.target.checked})}/> Best seller</label>
        </div><div className="admin-form-actions"><button className="button button-dark" disabled={busy}>{busy?"Saving…":editing?"Save changes":"Create product"}</button><button type="button" className="button button-outline" onClick={()=>{setEditing(null);setEditorOpen(false);setForm(blankProduct)}}>Cancel</button></div>
      </form>}
      <form className="admin-category-form" onSubmit={createCategory}><label>Add a category<input required maxLength={100} value={categoryName} onChange={e=>setCategoryName(e.target.value)} placeholder="Category name"/></label><button className="button button-outline" disabled={busy}>Add category</button></form>
      <div className="admin-category-list" aria-label="Manage categories">
        {categories.map(category=><div className="admin-category-row" key={category.slug}>
          <span>{category.name}<small>{category.active ? "Active" : "Removed"}</small></span>
          <button className="admin-inline-action" disabled={busy} onClick={()=>setCategoryActive(category,!category.active)}>{category.active ? "Remove" : "Restore"}</button>
        </div>)}
      </div>
      <label className="admin-search">Search products<input type="search" value={search} onChange={e=>setSearch(e.target.value)} placeholder="Name, slug, or SKU"/></label>
      <div className="admin-table-wrap"><table className="admin-table"><thead><tr><th>Product</th><th>Category</th><th>Price</th><th>Stock</th><th>Status</th><th>Actions</th></tr></thead><tbody>{products.map(p=><tr key={p.id}><td><div className="admin-product-cell">{p.images[0]&&<img src={p.images[0]} alt=""/>}<span><strong>{p.name}</strong><small>{p.sku||p.slug}</small></span></div></td><td>{p.category}{p.subcategory&&<small className="admin-subline">{p.subcategory}</small>}</td><td>{money(p.price)}</td><td>{p.stock}</td><td><span className={`admin-status ${p.status}`}>{p.status}</span></td><td><div className="admin-row-actions"><button onClick={()=>startEdit(p)}>Edit</button>{p.status!=="archived"&&<button onClick={()=>archiveProduct(p)} disabled={busy}>Archive</button>}</div></td></tr>)}{products.length===0&&<tr><td colSpan="6">No products found.</td></tr>}</tbody></table></div>
    </section>}

    {tab === "orders" && <section className="admin-workspace"><div className="admin-section-heading"><div><h2>Orders</h2><p>Review customer details and move orders through fulfillment.</p></div><button className="button button-outline" onClick={loadTab}>Refresh</button></div><div className="admin-order-list">{orders.map(order=><details className="admin-order" key={order.id}><summary><span><strong>{order.order_number}</strong><small>{order.customer_name} · {order.customer_email}</small></span><span className="admin-order-total">{money(Number(order.total))}</span><span className={`admin-status ${order.status}`}>{order.status}</span></summary><div className="admin-order-details"><div><h3>Items</h3>{(order.items||[]).map((item,i)=><p key={`${item.sku}-${i}`}>{item.product_name} × {item.quantity} · {money(Number(item.unit_price))} <small>{JSON.stringify(item.variant)}</small></p>)}</div><div><h3>Shipping</h3><pre>{JSON.stringify(order.shipping,null,2)}</pre><p>{order.payment_method} · {order.payment_status}</p></div><label>Update status<select value="" disabled={busy||!transitions[order.status]?.length} onChange={e=>e.target.value&&updateOrder(order,e.target.value)}><option value="">Choose next status</option>{(transitions[order.status]||[]).map(status=><option key={status}>{status}</option>)}</select></label></div></details>)}{orders.length===0&&<p>No orders have been placed yet.</p>}</div></section>}

    {tab === "customers" && <section className="admin-workspace"><div className="admin-section-heading"><div><h2>Customers</h2><p>Block or restore customer accounts. Administrator accounts are protected.</p></div><button className="button button-outline" onClick={loadTab}>Refresh</button></div><div className="admin-table-wrap"><table className="admin-table"><thead><tr><th>Name</th><th>Email</th><th>Joined</th><th>Status</th><th>Action</th></tr></thead><tbody>{customers.map(c=><tr key={c.id}><td>{c.full_name}</td><td>{c.email}</td><td>{new Date(c.created_at).toLocaleDateString()}</td><td><span className={`admin-status ${c.status}`}>{c.status}</span></td><td><button className="admin-inline-action" disabled={busy} onClick={()=>updateCustomer(c,c.status==="active"?"blocked":"active")}>{c.status==="active"?"Block":"Restore"}</button></td></tr>)}{customers.length===0&&<tr><td colSpan="5">No customers yet.</td></tr>}</tbody></table></div></section>}
  </main>;
}
