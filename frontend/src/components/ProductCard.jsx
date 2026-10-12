import {Link} from "react-router-dom"; import {money} from "../data/products";
export default function ProductCard({p,onQuickAdd}) {
  return <article className="product-card">
    <div className="product-image-wrap">
      <Link className="product-image-button" to={`/product/${p.id}`} aria-label={`View ${p.name}`}><img src={p.images[0]} alt={p.name} loading="lazy"/></Link>
      {p.tag&&<span className="product-badge">{p.tag}</span>}
      <button className="quick-add" onClick={()=>onQuickAdd(p)} aria-label={`Quick add ${p.name}`}>+</button>
    </div>
    <div className="product-card-main"><Link className="product-name-button" to={`/product/${p.id}`}>{p.name}</Link><span className="product-price">{money(p.price)}</span></div>
    <div className="product-meta"><span>{p.category}</span><span className="swatches">{p.colors.slice(0,3).map(c=><span className="swatch-dot" title={c[0]} style={{background:c[1]}} key={c[0]}/>)}</span></div>
  </article>
}
