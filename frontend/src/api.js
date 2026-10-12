const API_BASE = import.meta.env.VITE_API_BASE_URL || "http://localhost:8080/api/v1";

export async function api(path, options = {}) {
  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: { "Content-Type": "application/json", ...options.headers },
    credentials: "include",
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `Request failed (${response.status})`);
  }
  return response.json();
}

export const listProducts = (query = "") => api(`/products${query ? `?${query}` : ""}`);
export const listCategories = () => api("/categories");
