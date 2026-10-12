import ProductCard from "./ProductCard";
export default function ProductSlider({products,onQuickAdd,id}) {
  return <div className="product-slider" id={id}>{products.map(p=><ProductCard key={p.id} p={p} onQuickAdd={onQuickAdd}/>)}</div>
}
