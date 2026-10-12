import {useEffect,useState} from "react";
import {BrowserRouter,useLocation,useNavigate} from "react-router-dom";
import {Routes,Route} from "react-router-dom";
import Header from "./components/Header";
import CategorySidebar from "./components/CategorySidebar";
import Home from "./pages/Home";
import Shop from "./pages/Shop";
import ProductDetails from "./pages/ProductDetails";
import Admin from "./pages/Admin";
import {api,listCategories,listProducts} from "./api";
import {BagDrawer,TryOnModal,AccountModal,CheckoutModal} from "./components/Overlays";

function Store(){
 const nav=useNavigate(),loc=useLocation();
 const [catalog,setCatalog]=useState([]),[categories,setCategories]=useState([]),[catalogError,setCatalogError]=useState("");
 const [user,setUser]=useState(null),[authLoading,setAuthLoading]=useState(true);
 const [bag,setBag]=useState(()=>{try{return JSON.parse(localStorage.getItem("heebee-bag")||"[]")}catch{return[]}});
 const [sidebar,setSidebar]=useState(false),[bagOpen,setBagOpen]=useState(false),[tryon,setTryon]=useState(false),[account,setAccount]=useState(false),[checkout,setCheckout]=useState(false),[tryProduct,setTryProduct]=useState(null);
 useEffect(()=>{let live=true;Promise.all([listProducts(),listCategories()]).then(([p,c])=>{if(live){setCatalog(p);setCategories(c)}}).catch(e=>live&&setCatalogError(e.message));return()=>{live=false}},[]);
 useEffect(()=>{let live=true;api("/auth/me").then(data=>{if(live)setUser(data.user||data)}).catch(()=>{if(live)setUser(null)}).finally(()=>{if(live)setAuthLoading(false)});return()=>{live=false}},[]);
 const save=b=>{setBag(b);localStorage.setItem("heebee-bag",JSON.stringify(b.map(x=>({productId:x.productId,size:x.size,color:x.color,qty:x.qty}))))};
 const getProduct=id=>catalog.find(p=>p.id===id);
 const bagWithProducts=bag.map(x=>({...x,product:getProduct(x.productId)})).filter(x=>x.product);
 const add=(p,size,color,qty=1)=>{if(!size){alert("Choose a size first.");return}const b=[...bag],i=b.findIndex(x=>x.productId===p.id&&x.size===size&&x.color===color);if(i>=0)b[i].qty=Math.min(20,b[i].qty+qty);else b.push({productId:p.id,size,color,qty:Math.min(20,qty)});save(b)};
 const quick=p=>add(p,p.sizes.includes("M")?"M":p.sizes[0],p.colors[0]?.[0]||"",1);
 const change=(i,d)=>{const b=[...bag];b[i].qty+=d;if(b[i].qty<1)b.splice(i,1);if(b[i]&&b[i].qty>20)b[i].qty=20;save(b)};
 const remove=i=>{const b=[...bag];b.splice(i,1);save(b)};
 const search=q=>nav(q?`/shop?search=${encodeURIComponent(q)}`:"/shop");
 const params=new URLSearchParams(loc.search),query=params.get("search")||"",shopPath=loc.pathname.startsWith("/shop");
 const category=shopPath&&loc.pathname!=="/shop"?decodeURIComponent(loc.pathname.split("/").slice(2).join("/")):null;
 const adminRoute=loc.pathname==="/admin";
 return <div><Header user={user} bagCount={bag.reduce((s,x)=>s+x.qty,0)} onOpenBag={()=>setBagOpen(true)} onOpenAccount={()=>setAccount(true)} onOpenTryOn={()=>{setTryProduct(null);setTryon(true)}} onSearch={search} onOpenMenu={()=>setSidebar(true)}/>
 {catalogError&&<div role="status" className="main-content">The product service is unavailable: {catalogError}</div>}
 <div className={"page-shell "+(adminRoute?"admin-shell":"")}>{!adminRoute&&<CategorySidebar categories={categories} products={catalog} category={category} subcategory={null} mobileOpen={sidebar} onClose={()=>setSidebar(false)} onHome={()=>{setSidebar(false);nav("/")}} onCategory={c=>{setSidebar(false);nav(`/shop/${encodeURIComponent(c)}`)}} onSubcategory={s=>{setSidebar(false);nav(`/shop/${encodeURIComponent(category||"")}?subcategory=${encodeURIComponent(s)}`)}}/>}
 <Routes><Route path="/" element={<Home products={catalog} onQuickAdd={quick} onShop={()=>nav("/shop?sort=newest")} onTryOn={()=>{setTryProduct(null);setTryon(true)}} onCategory={c=>nav(`/shop/${encodeURIComponent(c)}`)}/>} />
 <Route path="/shop" element={<Shop query={query} category={null} subcategory={params.get("subcategory")} onQuickAdd={quick}/>} />
 <Route path="/shop/:category" element={<Shop query={query} category={category} subcategory={params.get("subcategory")} onQuickAdd={quick}/>} />
 <Route path="/product/:id" element={<ProductDetails onAdd={add} onTryOn={p=>{setTryProduct(p);setTryon(true)}} onQuickAdd={quick}/>} />
 <Route path="/admin" element={<Admin user={user} loading={authLoading}/>} />
 </Routes></div>
 <BagDrawer open={bagOpen} onClose={()=>setBagOpen(false)} bag={bagWithProducts} onQty={change} onRemove={remove} onCheckout={()=>{setBagOpen(false);setCheckout(true)}}/>
 <TryOnModal key={tryProduct?.id||"default"} open={tryon} onClose={()=>setTryon(false)} initialProduct={tryProduct} products={catalog}/><AccountModal open={account} onClose={()=>setAccount(false)} onAuthChange={setUser}/><CheckoutModal open={checkout} onClose={()=>setCheckout(false)} bag={bagWithProducts} user={user} onNeedLogin={()=>{setCheckout(false);setAccount(true)}} onPlaced={()=>save([])}/></div>
}
export default function App(){return <BrowserRouter><Store/></BrowserRouter>}
