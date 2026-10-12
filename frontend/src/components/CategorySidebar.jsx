export default function CategorySidebar({category,subcategory,onHome,onCategory,onSubcategory,mobileOpen,onClose,categories=[],products=[]}) {
  return <><aside className={"category-sidebar "+(mobileOpen?"mobile-open":"")} aria-label="Clothing categories">
    <div className="sidebar-topline"><span>EXPLORE</span><span className="tiny-cross">✳</span></div>
    <button className={"category-home "+(!category&&!subcategory?"active":"")} onClick={onHome}><span className="category-line-icon">↗</span> All pieces <span className="category-count">{products.length}</span></button>
    <div className="category-list">{categories.map(c=>{
      const open=category===c.name;
      return <div className={"category-group "+(open?"open":"")} key={c.name}>
        <div className="category-row">
          <button className="category-trigger" onClick={()=>onCategory(c.name)} aria-expanded={open}><span>{c.name}</span><span className="category-count">{products.filter(p=>p.category===c.name).length}</span></button>
          <button className="category-expand" onClick={()=>onCategory(c.name)} aria-label={"Expand "+c.name} aria-expanded={open}>›</button>
        </div>
        <div className="subcategory-list"><div className="subcategory-inner">{c.subs.map(s=><button key={s} className={"subcategory-button "+(subcategory===s?"active":"")} onClick={()=>onSubcategory(s)}>{s}</button>)}</div></div>
      </div>
    })}</div>
    <div className="sidebar-bottom"><div className="sidebar-note-mark">HB<span>®</span></div><p>Less noise.<br/><strong>More expression.</strong></p><a href="#about" className="sidebar-about">The HeeBee point of view <span>↗</span></a></div>
  </aside><div className={"mobile-sidebar-scrim "+(mobileOpen?"visible":"")} onClick={onClose}/></>;
}
