import { useState } from "react";
import { Link } from "react-router-dom";

export default function Header({bagCount,user,onOpenBag,onOpenAccount,onOpenTryOn,onSearch,onOpenMenu}) {
  const [query,setQuery]=useState("");
  const submit=e=>{e.preventDefault();onSearch(query)};
  return <>
    <div className="announcement-bar">
      <span>FREE SHIPPING ON ORDERS OVER ৳5,000</span>
      <span className="announcement-right">NEW DROP 06 <span>✳</span> NOW LIVE</span>
    </div>
    <header className="site-header">
      <button className="mobile-menu" onClick={onOpenMenu} aria-label="Open menu"><span className="hamburger"/></button>
      <Link to="/" className="brand" aria-label="HeeBee home">
        <span className="brand-mark"><span>H</span><span>B</span></span><span className="brand-name">HeeBee<span className="brand-period">.</span></span>
      </Link>
      <form className="search-form" onSubmit={submit} role="search">
        <span className="search-symbol" aria-hidden="true"><svg viewBox="0 0 24 24"><circle cx="10.8" cy="10.8" r="6.6"/><path d="m16 16 4.3 4.3"/></svg></span>
        <input value={query} onChange={e=>setQuery(e.target.value)} placeholder="Search the collection" aria-label="Search the collection"/>
        <kbd>⌘K</kbd>
      </form>
      <nav className="header-actions">
        <button className="text-action" onClick={onOpenTryOn}><span className="sparkle">✳</span><span>Try On</span></button>
        {user?.role==="admin"&&<Link className="text-action admin-nav-link" to="/admin">Admin</Link>}
        <button className="text-action" onClick={onOpenAccount}><span className="action-icon">♙</span><span>Account</span></button>
        <button className="bag-action" onClick={onOpenBag}><span className="bag-icon"><svg viewBox="0 0 24 24"><path d="M5 8h14l-1 12H6L5 8Z"/><path d="M9 8a3 3 0 0 1 6 0"/></svg></span><span className="bag-label">Bag</span><span className="bag-count">{bagCount>99?"99+":bagCount}</span></button>
      </nav>
    </header>
    <div className="mobile-search-wrap">
      <form className="search-form" onSubmit={submit} role="search">
        <span className="search-symbol"><svg viewBox="0 0 24 24"><circle cx="10.8" cy="10.8" r="6.6"/><path d="m16 16 4.3 4.3"/></svg></span>
        <input value={query} onChange={e=>setQuery(e.target.value)} placeholder="Search the collection" aria-label="Search the collection"/>
      </form>
    </div>
  </>;
}
