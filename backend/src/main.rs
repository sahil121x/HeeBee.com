use actix_cors::Cors;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, ResponseError,
    cookie::{Cookie, SameSite, time::Duration as CookieDuration},
    middleware, web,
};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{env, time::Duration};

#[derive(Clone)]
struct AppState {
    db: PgPool,
}

#[derive(Debug)]
struct ApiError(String);
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl ResponseError for ApiError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    }
    fn error_response(&self) -> HttpResponse {
        HttpResponse::InternalServerError()
            .json(serde_json::json!({"error":"internal server error"}))
    }
}

#[derive(Serialize)]
struct Product {
    id: String,
    name: String,
    slug: String,
    price: f64,
    category: String,
    subcategory: Option<String>,
    description: String,
    details: String,
    tag: Option<String>,
    #[serde(rename = "isNew")]
    is_new: bool,
    best: bool,
    colors: serde_json::Value,
    sizes: serde_json::Value,
    images: Vec<String>,
}

#[derive(Serialize)]
struct Category {
    name: String,
    subs: Vec<String>,
}

#[derive(Deserialize)]
struct ProductQuery {
    search: Option<String>,
    category: Option<String>,
    subcategory: Option<String>,
    min_price: Option<f64>,
    max_price: Option<f64>,
    featured: Option<bool>,
    sort: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status":"ok"}))
}

async fn products(
    state: web::Data<AppState>,
    query: web::Query<ProductQuery>,
) -> Result<HttpResponse, ApiError> {
    let limit = query.limit.unwrap_or(48).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let search = query
        .search
        .as_deref()
        .map(|s| format!("%{}%", s.replace('%', "\\%").replace('_', "\\_")));
    let rows = sqlx::query_as::<_, (String,String,String,f64,String,Option<String>,String,String,Option<String>,bool,bool,serde_json::Value,serde_json::Value,Vec<String>)>(
        "SELECT p.id::text,p.name,p.slug,p.price::float8,c.name,p.subcategory,p.description,p.details,p.tag,p.is_new,p.best,p.colors,p.sizes,COALESCE(array_agg(pi.url ORDER BY pi.position) FILTER (WHERE pi.url IS NOT NULL), ARRAY[]::text[]) FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN product_images pi ON pi.product_id=p.id WHERE p.status='published' AND ($1::text IS NULL OR p.name ILIKE $1 OR p.description ILIKE $1 OR c.name ILIKE $1) AND ($2::text IS NULL OR c.slug=$2 OR c.name=$2) AND ($3::text IS NULL OR p.subcategory=$3) AND ($4::float8 IS NULL OR p.price >= $4) AND ($5::float8 IS NULL OR p.price <= $5) AND ($6::bool IS NULL OR p.best=$6) GROUP BY p.id,c.name ORDER BY CASE WHEN $7='featured' THEN p.best END DESC, CASE WHEN $7='price-asc' THEN p.price END ASC, CASE WHEN $7='price-desc' THEN p.price END DESC, CASE WHEN $7='name' THEN p.name END ASC, p.created_at DESC LIMIT $8 OFFSET $9"
    ).bind(search).bind(query.category.as_deref()).bind(query.subcategory.as_deref()).bind(query.min_price).bind(query.max_price).bind(query.featured).bind(query.sort.as_deref().unwrap_or("featured")).bind(limit).bind(offset).fetch_all(&state.db).await.map_err(|e| { tracing::error!(error=%e,"product query failed"); ApiError("database query failed".into()) })?;
    let list: Vec<Product> = rows
        .into_iter()
        .map(|r| Product {
            id: r.0.clone(),
            name: r.1,
            slug: r.2,
            price: r.3,
            category: r.4,
            subcategory: r.5,
            description: r.6,
            details: r.7,
            tag: r.8,
            is_new: r.9,
            best: r.10,
            colors: r.11,
            sizes: r.12,
            images: r.13,
        })
        .collect();
    Ok(HttpResponse::Ok().json(list))
}

async fn product(
    state: web::Data<AppState>,
    slug: web::Path<String>,
) -> Result<HttpResponse, ApiError> {
    let rows = sqlx::query_as::<_, (String,String,String,f64,String,Option<String>,String,String,Option<String>,bool,bool,serde_json::Value,serde_json::Value,Vec<String>)>(
        "SELECT p.id::text,p.name,p.slug,p.price::float8,c.name,p.subcategory,p.description,p.details,p.tag,p.is_new,p.best,p.colors,p.sizes,COALESCE(array_agg(pi.url ORDER BY pi.position) FILTER (WHERE pi.url IS NOT NULL), ARRAY[]::text[]) FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN product_images pi ON pi.product_id=p.id WHERE p.status='published' AND (p.slug=$1 OR p.id::text=$1) GROUP BY p.id,c.name"
    ).bind(slug.into_inner()).fetch_optional(&state.db).await.map_err(|e| { tracing::error!(error=%e,"product query failed"); ApiError("database query failed".into()) })?;
    match rows {
        Some(r) => Ok(HttpResponse::Ok().json(Product {
            id: r.0.clone(),
            name: r.1,
            slug: r.2,
            price: r.3,
            category: r.4,
            subcategory: r.5,
            description: r.6,
            details: r.7,
            tag: r.8,
            is_new: r.9,
            best: r.10,
            colors: r.11,
            sizes: r.12,
            images: r.13,
        })),
        None => Ok(HttpResponse::NotFound().json(serde_json::json!({"error":"product not found"}))),
    }
}

async fn categories(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let rows = sqlx::query_as::<_, (String, Vec<String>)>("SELECT c.name, COALESCE(array_agg(DISTINCT p.subcategory) FILTER (WHERE p.subcategory IS NOT NULL), ARRAY[]::text[]) FROM categories c LEFT JOIN products p ON p.category_id=c.id AND p.status='published' WHERE c.active GROUP BY c.id ORDER BY c.position,c.name").fetch_all(&state.db).await.map_err(|e| { tracing::error!(error=%e,"category query failed"); ApiError("database query failed".into()) })?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|(name, subs)| Category { name, subs })
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
struct RegisterInput {
    full_name: String,
    email: String,
    password: String,
}
#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}
#[derive(Deserialize)]
struct VerifyInput {
    email: String,
    code: String,
}
#[derive(Deserialize)]
struct GoogleInput {
    credential: String,
}

fn normalize_email(email: &str) -> Option<String> {
    let value = email.trim().to_ascii_lowercase();
    let (local, domain) = value.split_once('@')?;
    if local.is_empty()
        || domain.len() < 3
        || !domain.contains('.')
        || value.len() > 254
        || value.chars().any(char::is_whitespace)
    {
        return None;
    }
    const DISPOSABLE: &[&str] = &[
        "mailinator.com",
        "tempmail.com",
        "10minutemail.com",
        "guerrillamail.com",
        "yopmail.com",
    ];
    if DISPOSABLE.contains(&domain) {
        return None;
    }
    Some(value)
}
fn hash_value(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
fn is_admin(email: &str) -> bool {
    std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|v| v.trim().to_ascii_lowercase())
        .any(|v| !v.is_empty() && v == email)
}
fn effective_role(email: &str) -> &'static str {
    if is_admin(email) { "admin" } else { "customer" }
}
fn auth_error(status: actix_web::http::StatusCode, message: &str) -> HttpResponse {
    HttpResponse::build(status).json(serde_json::json!({"error":message}))
}
fn session_cookie(token: String, ttl: i64) -> Cookie<'static> {
    Cookie::build("heebee_session", token)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(std::env::var("APP_ENV").as_deref() == Ok("production"))
        .path("/")
        .max_age(CookieDuration::seconds(ttl))
        .finish()
}
async fn create_session(pool: &PgPool, user_id: uuid::Uuid) -> Result<(String, i64), sqlx::Error> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes);
    let ttl = std::env::var("JWT_REFRESH_TOKEN_TTL")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(2_592_000)
        .clamp(60, 31_536_000);
    sqlx::query("INSERT INTO user_sessions(user_id,token_hash,expires_at) VALUES($1,$2,now()+($3 * interval '1 second'))")
        .bind(user_id).bind(hash_value(&token)).bind(ttl).execute(pool).await?;
    Ok((token, ttl))
}
async fn session_user(
    req: &HttpRequest,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, String, String, String)>, sqlx::Error> {
    let Some(token) = req.cookie("heebee_session").map(|c| c.value().to_owned()) else {
        return Ok(None);
    };
    let user=sqlx::query_as::<_,(uuid::Uuid,String,String,String)>("SELECT u.id,u.email,u.full_name,u.role FROM user_sessions s JOIN users u ON u.id=s.user_id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>now() AND u.status='active'")
        .bind(hash_value(&token)).fetch_optional(pool).await?;
    Ok(user.map(|(id, email, name, _stored_role)| {
        (id, email.clone(), name, effective_role(&email).to_owned())
    }))
}
async fn send_verification_email(email: &str, code: &str) -> Result<(), String> {
    let key = std::env::var("RESEND_API_KEY")
        .map_err(|_| "Email verification is not configured on the server.".to_owned())?;
    let from = std::env::var("RESEND_FROM_EMAIL")
        .map_err(|_| "Email verification is not configured on the server.".to_owned())?;
    let response = reqwest::Client::new().post("https://api.resend.com/emails").bearer_auth(key)
        .json(&serde_json::json!({"from":from,"to":[email],"subject":"Your HeeBee verification code","text":format!("Your verification code is {code}. It expires in 10 minutes.")}))
        .send().await.map_err(|_| "Could not send verification email.".to_owned())?;
    if !response.status().is_success() {
        tracing::error!(status=%response.status(),"email provider rejected verification email");
        return Err("Could not send verification email.".to_owned());
    }
    Ok(())
}

async fn register(state: web::Data<AppState>, body: web::Json<RegisterInput>) -> HttpResponse {
    if std::env::var("RESEND_API_KEY").is_err() || std::env::var("RESEND_FROM_EMAIL").is_err() {
        return auth_error(
            actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
            "Email verification is not configured on the server.",
        );
    }
    let Some(email) = normalize_email(&body.email) else {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Enter a valid, non-disposable email address.",
        );
    };
    let name = body.full_name.trim();
    if name.is_empty()
        || name.len() > 120
        || body.password.len() < 8
        || body.password.len() > 128
        || body.password.chars().all(char::is_whitespace)
    {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Check your name and password. Passwords must be 8–128 characters.",
        );
    }
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = match Argon2::default().hash_password(body.password.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create account.",
            );
        }
    };
    let mut bytes = [0u8; 4];
    OsRng.fill_bytes(&mut bytes);
    let code = format!("{:06}", u32::from_be_bytes(bytes) % 1_000_000);
    let code_salt = SaltString::generate(&mut OsRng);
    let code_hash = match Argon2::default().hash_password(code.as_bytes(), &code_salt) {
        Ok(h) => h.to_string(),
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create account.",
            );
        }
    };
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create account.",
            );
        }
    };
    let inserted=sqlx::query_scalar::<_,uuid::Uuid>("INSERT INTO users(full_name,email,password_hash,status) VALUES($1,$2,$3,'pending') ON CONFLICT(email) DO NOTHING RETURNING id")
        .bind(name).bind(&email).bind(password_hash).fetch_optional(&mut *tx).await;
    let user_id = match inserted {
        Ok(Some(id)) => id,
        Ok(None) => {
            let _ = tx.rollback().await;
            return HttpResponse::Ok().json(serde_json::json!({"requires_verification":true,"message":"If the address can be registered, a verification code will be sent."}));
        }
        Err(_) => {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create account.",
            );
        }
    };
    let saved=sqlx::query("INSERT INTO email_verification_codes(user_id,code_hash,expires_at) VALUES($1,$2,now()+interval '10 minutes')").bind(user_id).bind(code_hash).execute(&mut *tx).await;
    if saved.is_err() || tx.commit().await.is_err() {
        return auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not create account.",
        );
    }
    match send_verification_email(&email, &code).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"requires_verification":true})),
        Err(e) => {
            tracing::error!("verification email delivery failed");
            let _ = sqlx::query("DELETE FROM users WHERE id=$1 AND status='pending'")
                .bind(user_id)
                .execute(&state.db)
                .await;
            auth_error(actix_web::http::StatusCode::SERVICE_UNAVAILABLE, &e)
        }
    }
}

async fn verify_email(state: web::Data<AppState>, body: web::Json<VerifyInput>) -> HttpResponse {
    let Some(email) = normalize_email(&body.email) else {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid email or verification code.",
        );
    };
    if body.code.len() != 6 || !body.code.bytes().all(|b| b.is_ascii_digit()) {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid email or verification code.",
        );
    }
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not verify email.",
            );
        }
    };
    let row=sqlx::query_as::<_,(uuid::Uuid,String)>("SELECT v.id,v.code_hash FROM email_verification_codes v JOIN users u ON u.id=v.user_id WHERE u.email=$1 AND u.status='pending' AND v.consumed_at IS NULL AND v.expires_at>now() AND v.attempts<5 ORDER BY v.created_at DESC LIMIT 1 FOR UPDATE OF v")
        .bind(&email).fetch_optional(&mut *tx).await;
    let Some((code_id, hash)) = row.ok().flatten() else {
        let _ = tx.rollback().await;
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid email or verification code.",
        );
    };
    let parsed = PasswordHash::new(&hash);
    let valid = parsed.ok().is_some_and(|h| {
        Argon2::default()
            .verify_password(body.code.as_bytes(), &h)
            .is_ok()
    });
    if !valid {
        let _ = sqlx::query("UPDATE email_verification_codes SET attempts=attempts+1 WHERE id=$1")
            .bind(code_id)
            .execute(&mut *tx)
            .await;
        let _ = tx.commit().await;
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid email or verification code.",
        );
    }
    let _ = sqlx::query("UPDATE email_verification_codes SET consumed_at=now() WHERE id=$1")
        .bind(code_id)
        .execute(&mut *tx)
        .await;
    let updated=sqlx::query("UPDATE users SET status='active',email_verified_at=now(),role=CASE WHEN $2 THEN 'admin' ELSE 'customer' END,updated_at=now() WHERE id=(SELECT user_id FROM email_verification_codes WHERE id=$1)").bind(code_id).bind(is_admin(&email)).execute(&mut *tx).await;
    if updated.is_err() || tx.commit().await.is_err() {
        return auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not verify email.",
        );
    }
    HttpResponse::Ok().json(serde_json::json!({"verified":true}))
}

async fn login(state: web::Data<AppState>, body: web::Json<LoginInput>) -> HttpResponse {
    let Some(email) = normalize_email(&body.email) else {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Email or password is incorrect.",
        );
    };
    let row=sqlx::query_as::<_,(uuid::Uuid,String,String,String,String)>("SELECT id,email,full_name,role,password_hash FROM users WHERE email=$1 AND status='active'").bind(email).fetch_optional(&state.db).await;
    let Some((id, email, name, _stored_role, hash)) = row.ok().flatten() else {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Email or password is incorrect.",
        );
    };
    let parsed = PasswordHash::new(&hash);
    if !parsed.ok().is_some_and(|h| {
        Argon2::default()
            .verify_password(body.password.as_bytes(), &h)
            .is_ok()
    }) {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Email or password is incorrect.",
        );
    }
    let role = effective_role(&email);
    match create_session(&state.db, id).await {
        Ok((token, ttl)) => HttpResponse::Ok()
            .cookie(session_cookie(token, ttl))
            .json(serde_json::json!({"user":{"id":id,"email":email,"full_name":name,"role":role}})),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not sign in.",
        ),
    }
}

async fn me(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    match session_user(&req, &state.db).await {
        Ok(Some((id, email, name, role))) => HttpResponse::Ok()
            .json(serde_json::json!({"user":{"id":id,"email":email,"full_name":name,"role":role}})),
        Ok(None) => auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Sign in to continue.",
        ),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not read session.",
        ),
    }
}
async fn logout(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if let Some(cookie) = req.cookie("heebee_session") {
        let _ = sqlx::query(
            "UPDATE user_sessions SET revoked_at=now() WHERE token_hash=$1 AND revoked_at IS NULL",
        )
        .bind(hash_value(cookie.value()))
        .execute(&state.db)
        .await;
    }
    HttpResponse::Ok()
        .cookie(
            Cookie::build("heebee_session", "")
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(std::env::var("APP_ENV").as_deref() == Ok("production"))
                .path("/")
                .max_age(CookieDuration::seconds(0))
                .finish(),
        )
        .json(serde_json::json!({"logged_out":true}))
}

#[derive(Deserialize)]
struct GoogleClaims {
    iss: String,
    aud: String,
    sub: String,
    email: String,
    name: Option<String>,
    email_verified: serde_json::Value,
    exp: String,
}
async fn google_login(state: web::Data<AppState>, body: web::Json<GoogleInput>) -> HttpResponse {
    let client_id = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(v) => v,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
                "Google sign-in is not configured.",
            );
        }
    };
    let response = match reqwest::Client::new()
        .get("https://oauth2.googleapis.com/tokeninfo")
        .query(&[("id_token", body.credential.as_str())])
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::BAD_GATEWAY,
                "Google sign-in could not be verified.",
            );
        }
    };
    if !response.status().is_success() {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Google sign-in could not be verified.",
        );
    }
    let claims = match response.json::<GoogleClaims>().await {
        Ok(v) => v,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::UNAUTHORIZED,
                "Google sign-in could not be verified.",
            );
        }
    };
    let exp = claims.exp.parse::<i64>().unwrap_or(0);
    let verified = claims.email_verified == serde_json::Value::Bool(true)
        || claims.email_verified.as_str() == Some("true");
    if !["accounts.google.com", "https://accounts.google.com"].contains(&claims.iss.as_str())
        || claims.aud != client_id
        || exp <= chrono::Utc::now().timestamp()
        || !verified
        || claims.sub.is_empty()
    {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Google sign-in could not be verified.",
        );
    }
    let Some(email) = normalize_email(&claims.email) else {
        return auth_error(
            actix_web::http::StatusCode::UNAUTHORIZED,
            "Google sign-in could not be verified.",
        );
    };
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not sign in.",
            );
        }
    };
    let existing = sqlx::query_as::<_, (uuid::Uuid, String, String, String, Option<String>, String)>(
        "SELECT id,email,full_name,role,google_subject,status FROM users WHERE email=$1 FOR UPDATE",
    )
    .bind(&email)
    .fetch_optional(&mut *tx)
    .await;
    let user = match existing {
        Ok(Some((id, e, name, role, Some(sub), status)))
            if sub == claims.sub && status == "active" =>
        {
            Some((id, e, name, role))
        }
        Ok(Some(_)) => {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::CONFLICT,
                "This email already has an account using another sign-in method.",
            );
        }
        Ok(None) => {
            let role = if is_admin(&email) {
                "admin"
            } else {
                "customer"
            };
            sqlx::query_as::<_,(uuid::Uuid,String,String,String)>("INSERT INTO users(full_name,email,role,status,email_verified_at,google_subject) VALUES($1,$2,$3,'active',now(),$4) RETURNING id,email,full_name,role").bind(claims.name.as_deref().filter(|n|!n.trim().is_empty()).unwrap_or("HeeBee customer")).bind(&email).bind(role).bind(&claims.sub).fetch_optional(&mut *tx).await.ok().flatten()
        }
        _ => {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not sign in.",
            );
        }
    };
    let Some((id, email, name, _stored_role)) = user else {
        let _ = tx.rollback().await;
        return auth_error(
            actix_web::http::StatusCode::CONFLICT,
            "This email cannot be linked to Google sign-in.",
        );
    };
    if tx.commit().await.is_err() {
        return auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not sign in.",
        );
    }
    let role = effective_role(&email);
    match create_session(&state.db, id).await {
        Ok((token, ttl)) => HttpResponse::Ok()
            .cookie(session_cookie(token, ttl))
            .json(serde_json::json!({"user":{"id":id,"email":email,"full_name":name,"role":role}})),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not sign in.",
        ),
    }
}

#[derive(Deserialize)]
struct CheckoutLine {
    product_id: uuid::Uuid,
    quantity: i32,
    size: String,
    color: String,
}
#[derive(Deserialize)]
struct ShippingInput {
    recipient: String,
    phone: String,
    street: String,
    city: String,
    postal_code: String,
    country: String,
    instructions: Option<String>,
}
#[derive(Deserialize)]
struct CheckoutInput {
    items: Vec<CheckoutLine>,
    shipping: ShippingInput,
}

async fn checkout(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CheckoutInput>,
) -> HttpResponse {
    let (user_id, email, _, _) = match session_user(&req, &state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return auth_error(
                actix_web::http::StatusCode::UNAUTHORIZED,
                "Sign in with a verified account before checkout.",
            );
        }
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not verify your session.",
            );
        }
    };
    let shipping = &body.shipping;
    if body.items.is_empty()
        || body.items.len() > 50
        || body
            .items
            .iter()
            .any(|i| i.quantity < 1 || i.quantity > 20 || i.size.len() > 30 || i.color.len() > 80)
        || shipping.recipient.trim().is_empty()
        || shipping.recipient.len() > 120
        || shipping.phone.trim().is_empty()
        || shipping.phone.len() > 40
        || shipping.street.trim().is_empty()
        || shipping.street.len() > 500
        || shipping.city.trim().is_empty()
        || shipping.city.len() > 100
        || shipping.postal_code.len() > 30
        || shipping.country.trim().is_empty()
        || shipping.country.len() > 100
        || shipping
            .instructions
            .as_deref()
            .is_some_and(|s| s.len() > 500)
    {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Check the delivery details and item quantities.",
        );
    }
    let key = req
        .headers()
        .get("Idempotency-Key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty() && v.len() <= 100)
        .map(str::to_owned);
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not start checkout.",
            );
        }
    };
    if let Some(ref key) = key
        && let Ok(Some((number, total))) = sqlx::query_as::<_, (String, String)>(
            "SELECT order_number,total::text FROM orders WHERE user_id=$1 AND idempotency_key=$2",
        )
        .bind(user_id)
        .bind(key)
        .fetch_optional(&mut *tx)
        .await
    {
        let _ = tx.rollback().await;
        return HttpResponse::Ok()
            .json(serde_json::json!({"order_number":number,"total":total,"duplicate":true}));
    }
    let mut subtotal = 0.0f64;
    let mut snapshots = Vec::new();
    for line in &body.items {
        let product = sqlx::query_as::<_, (String, Option<String>, String, i32, String)>(
            "SELECT name,sku,price::text,stock,status FROM products WHERE id=$1 FOR UPDATE",
        )
        .bind(line.product_id)
        .fetch_optional(&mut *tx)
        .await;
        let Ok(Some((name, sku, price, stock, status))) = product else {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::BAD_REQUEST,
                "One or more products are unavailable.",
            );
        };
        if status != "published" || stock < line.quantity {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::CONFLICT,
                "One or more products are unavailable or out of stock.",
            );
        }
        let Ok(unit_price) = price.parse::<f64>() else {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not calculate order total.",
            );
        };
        subtotal += unit_price * f64::from(line.quantity);
        let changed=sqlx::query("UPDATE products SET stock=stock-$2,updated_at=now() WHERE id=$1 AND stock >= $2 AND status='published'").bind(line.product_id).bind(line.quantity).execute(&mut *tx).await;
        if changed.is_err() {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::CONFLICT,
                "Could not reserve product inventory.",
            );
        }
        snapshots.push((
            line.product_id,
            name,
            sku,
            price,
            line.quantity,
            line.size.clone(),
            line.color.clone(),
        ));
    }
    let shipping_cost = if subtotal >= 5000.0 { 0.0 } else { 120.0 };
    let total = subtotal + shipping_cost;
    let order_number = format!(
        "HB-{}-{}",
        chrono::Utc::now().format("%Y%m%d"),
        uuid::Uuid::new_v4().simple().to_string()[..8].to_ascii_uppercase()
    );
    let shipping_json = serde_json::json!({"recipient":shipping.recipient.trim(),"email":email,"phone":shipping.phone.trim(),"street":shipping.street.trim(),"city":shipping.city.trim(),"postal_code":shipping.postal_code.trim(),"country":shipping.country.trim(),"instructions":shipping.instructions.as_deref().unwrap_or("").trim()});
    let order=sqlx::query_scalar::<_,uuid::Uuid>("INSERT INTO orders(order_number,user_id,status,payment_method,payment_status,shipping_snapshot,subtotal,shipping,total,idempotency_key) VALUES($1,$2,'pending','cash_on_delivery','unpaid',$3,$4::numeric,$5::numeric,$6::numeric,$7) RETURNING id")
        .bind(&order_number).bind(user_id).bind(shipping_json).bind(format!("{subtotal:.2}")).bind(format!("{shipping_cost:.2}")).bind(format!("{total:.2}")).bind(key).fetch_one(&mut *tx).await;
    let order_id = match order {
        Ok(id) => id,
        Err(_) => {
            let _ = tx.rollback().await;
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create order.",
            );
        }
    };
    for (product_id, name, sku, price, quantity, size, color) in snapshots {
        let variant = serde_json::json!({"size":size,"color":color});
        if sqlx::query("INSERT INTO order_items(order_id,product_id,product_name,sku,variant,quantity,unit_price) VALUES($1,$2,$3,$4,$5,$6,$7::numeric)").bind(order_id).bind(product_id).bind(name).bind(sku).bind(variant).bind(quantity).bind(price).execute(&mut *tx).await.is_err(){let _=tx.rollback().await;return auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not save order items.")}
    }
    if tx.commit().await.is_err() {
        return auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not complete checkout.",
        );
    }
    HttpResponse::Created().json(serde_json::json!({"id":order_id,"order_number":order_number,"subtotal":format!("{subtotal:.2}"),"shipping":format!("{shipping_cost:.2}"),"total":format!("{total:.2}"),"payment_method":"cash_on_delivery","status":"pending"}))
}

async fn customer_orders(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let (user_id, _, _, _) = match session_user(&req, &state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return auth_error(
                actix_web::http::StatusCode::UNAUTHORIZED,
                "Sign in to view orders.",
            );
        }
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not read orders.",
            );
        }
    };
    match sqlx::query_as::<_,(String,String,String,String,String,chrono::DateTime<chrono::Utc>,serde_json::Value)>("SELECT o.id::text,o.order_number,o.status,o.total::text,o.payment_status,o.created_at,COALESCE((SELECT jsonb_agg(jsonb_build_object('product_name',oi.product_name,'quantity',oi.quantity,'unit_price',oi.unit_price::text,'variant',oi.variant)) FROM order_items oi WHERE oi.order_id=o.id),'[]'::jsonb) FROM orders o WHERE o.user_id=$1 ORDER BY o.created_at DESC").bind(user_id).fetch_all(&state.db).await{Ok(rows)=>HttpResponse::Ok().json(rows.into_iter().map(|r|serde_json::json!({"id":r.0,"order_number":r.1,"status":r.2,"total":r.3,"payment_status":r.4,"created_at":r.5,"items":r.6})).collect::<Vec<_>>()),Err(_)=>auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not load orders.")}
}

async fn admin_id(req: &HttpRequest, pool: &PgPool) -> Result<uuid::Uuid, Box<HttpResponse>> {
    match session_user(req, pool).await {
        Ok(Some((id, _, _, role))) if role == "admin" => Ok(id),
        Ok(_) => Err(Box::new(auth_error(
            actix_web::http::StatusCode::FORBIDDEN,
            "Administrator access required.",
        ))),
        Err(_) => Err(Box::new(auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not authorize request.",
        ))),
    }
}

async fn audit(pool: &PgPool, actor: uuid::Uuid, action: &str, kind: &str, target: &str) {
    let _ = sqlx::query(
        "INSERT INTO audit_logs(actor_id,action,target_type,target_id) VALUES($1,$2,$3,$4)",
    )
    .bind(actor)
    .bind(action)
    .bind(kind)
    .bind(target)
    .execute(pool)
    .await;
}

async fn admin_dashboard(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(response) => return *response,
    };
    let result = sqlx::query_as::<_, (i64,i64,i64,i64)>("SELECT (SELECT count(*) FROM products WHERE status='published'),(SELECT count(*) FROM orders),(SELECT count(*) FROM users WHERE role='customer'),(SELECT count(*) FROM orders WHERE status='pending')")
        .fetch_one(&state.db).await;
    match result { Ok((products,orders,customers,pending_orders)) => HttpResponse::Ok().json(serde_json::json!({"products":products,"orders":orders,"customers":customers,"pending_orders":pending_orders,"admin_id":actor})), Err(_) => auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not load dashboard.") }
}

#[derive(Serialize, sqlx::FromRow)]
struct AdminProduct {
    id: String,
    name: String,
    slug: String,
    sku: Option<String>,
    price: f64,
    compare_at_price: Option<f64>,
    category: String,
    subcategory: Option<String>,
    description: String,
    details: String,
    tag: Option<String>,
    #[serde(rename = "isNew")]
    is_new: bool,
    best: bool,
    colors: serde_json::Value,
    sizes: serde_json::Value,
    images: Vec<String>,
    stock: i32,
    status: String,
}
async fn admin_products(
    req: HttpRequest,
    state: web::Data<AppState>,
    query: web::Query<ProductQuery>,
) -> HttpResponse {
    if let Err(response) = admin_id(&req, &state.db).await {
        return *response;
    }
    let term = query
        .search
        .as_deref()
        .map(|v| format!("%{}%", v.replace('%', "\\%").replace('_', "\\_")));
    let rows=sqlx::query_as::<_,AdminProduct>(
        "SELECT p.id::text AS id,p.name,p.slug,p.sku,p.price::float8 AS price,p.compare_at_price::float8 AS compare_at_price,c.name AS category,p.subcategory,p.description,p.details,p.tag,p.is_new,p.best,p.colors,p.sizes,COALESCE(array_agg(pi.url ORDER BY pi.position) FILTER(WHERE pi.url IS NOT NULL),ARRAY[]::text[]) AS images,p.stock,p.status FROM products p JOIN categories c ON c.id=p.category_id LEFT JOIN product_images pi ON pi.product_id=p.id WHERE ($1::text IS NULL OR p.name ILIKE $1 OR p.sku ILIKE $1 OR p.slug ILIKE $1) GROUP BY p.id,c.name ORDER BY p.updated_at DESC LIMIT $2 OFFSET $3")
        .bind(term).bind(query.limit.unwrap_or(50).clamp(1,100)).bind(query.offset.unwrap_or(0).max(0)).fetch_all(&state.db).await;
    match rows {
        Ok(rows) => HttpResponse::Ok().json(rows),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load products.",
        ),
    }
}

#[derive(Deserialize)]
struct ProductInput {
    name: String,
    slug: Option<String>,
    sku: Option<String>,
    category: String,
    subcategory: Option<String>,
    description: String,
    details: Option<String>,
    tag: Option<String>,
    price: f64,
    compare_at_price: Option<f64>,
    colors: serde_json::Value,
    sizes: serde_json::Value,
    images: Vec<String>,
    stock: i32,
    status: String,
    is_new: bool,
    best: bool,
}
fn slugify(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
fn can_transition_order(from: &str, to: &str) -> bool {
    match from {
        "pending" => matches!(to, "confirmed" | "cancelled"),
        "confirmed" => matches!(to, "processing" | "cancelled"),
        "processing" => matches!(to, "shipped" | "cancelled"),
        "shipped" => to == "delivered",
        "delivered" => to == "refunded",
        _ => false,
    }
}
async fn save_admin_product(
    pool: &PgPool,
    actor: uuid::Uuid,
    id: Option<uuid::Uuid>,
    item: ProductInput,
) -> Result<(), String> {
    let name = item.name.trim();
    let slug = slugify(
        item.slug
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(name),
    );
    if name.is_empty()
        || name.len() > 180
        || slug.is_empty()
        || item.price < 0.0
        || !item.price.is_finite()
        || item
            .compare_at_price
            .is_some_and(|v| !v.is_finite() || v < item.price)
        || item.stock < 0
        || !matches!(item.status.as_str(), "draft" | "published" | "archived")
        || !item.colors.is_array()
        || !item.sizes.is_array()
        || item.images.len() > 12
        || item
            .images
            .iter()
            .any(|url| url.len() > 2048 || !url.starts_with("https://"))
    {
        return Err(
            "Check the product fields, price, inventory, status, and HTTPS image URLs.".into(),
        );
    }
    let category = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT id FROM categories WHERE active AND (slug=$1 OR name=$2)",
    )
    .bind(slugify(&item.category))
    .bind(item.category.trim())
    .fetch_optional(pool)
    .await
    .map_err(|_| "Could not save product.".to_owned())?
    .ok_or_else(|| "Choose an active category.".to_owned())?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "Could not save product.".to_owned())?;
    let product_id = if let Some(id) = id {
        let result=sqlx::query("UPDATE products SET name=$2,slug=$3,sku=$4,category_id=$5,subcategory=$6,description=$7,details=$8,tag=$9,price=$10::numeric,compare_at_price=$11::text::numeric,colors=$12,sizes=$13,is_new=$14,best=$15,stock=$16,status=$17,updated_at=now() WHERE id=$1")
            .bind(id).bind(name).bind(&slug).bind(item.sku.as_deref().filter(|s|!s.trim().is_empty())).bind(category).bind(item.subcategory.as_deref()).bind(item.description.trim()).bind(item.details.as_deref().unwrap_or("")).bind(item.tag.as_deref().filter(|s|!s.trim().is_empty())).bind(item.price.to_string()).bind(item.compare_at_price.map(|v|v.to_string())).bind(&item.colors).bind(&item.sizes).bind(item.is_new).bind(item.best).bind(item.stock).bind(&item.status).execute(&mut *tx).await.map_err(|e|if e.to_string().contains("duplicate key") {"That slug or SKU is already in use.".to_owned()}else{"Could not save product.".to_owned()})?;
        if result.rows_affected() == 0 {
            return Err("Product not found.".into());
        }
        id
    } else {
        sqlx::query_scalar::<_,uuid::Uuid>("INSERT INTO products(name,slug,sku,category_id,subcategory,description,details,tag,price,compare_at_price,colors,sizes,is_new,best,stock,status) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9::numeric,$10::text::numeric,$11,$12,$13,$14,$15,$16) RETURNING id")
            .bind(name).bind(&slug).bind(item.sku.as_deref().filter(|s|!s.trim().is_empty())).bind(category).bind(item.subcategory.as_deref()).bind(item.description.trim()).bind(item.details.as_deref().unwrap_or("")).bind(item.tag.as_deref().filter(|s|!s.trim().is_empty())).bind(item.price.to_string()).bind(item.compare_at_price.map(|v|v.to_string())).bind(&item.colors).bind(&item.sizes).bind(item.is_new).bind(item.best).bind(item.stock).bind(&item.status).fetch_one(&mut *tx).await.map_err(|e|if e.to_string().contains("duplicate key") {"That slug or SKU is already in use.".to_owned()}else{"Could not save product.".to_owned()})?
    };
    sqlx::query("DELETE FROM product_images WHERE product_id=$1")
        .bind(product_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| "Could not save product images.".to_owned())?;
    for (position, url) in item.images.iter().enumerate() {
        sqlx::query("INSERT INTO product_images(product_id,url,source,position) VALUES($1,$2,'external',$3)").bind(product_id).bind(url).bind(position as i32).execute(&mut *tx).await.map_err(|_|"Could not save product images.".to_owned())?;
    }
    tx.commit()
        .await
        .map_err(|_| "Could not save product.".to_owned())?;
    audit(
        pool,
        actor,
        if id.is_some() {
            "product.updated"
        } else {
            "product.created"
        },
        "product",
        &product_id.to_string(),
    )
    .await;
    Ok(())
}
async fn create_admin_product(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<ProductInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match save_admin_product(&state.db, actor, None, body.into_inner()).await {
        Ok(()) => HttpResponse::Created().json(serde_json::json!({"saved":true})),
        Err(e) => auth_error(actix_web::http::StatusCode::BAD_REQUEST, &e),
    }
}
async fn update_admin_product(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<uuid::Uuid>,
    body: web::Json<ProductInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match save_admin_product(&state.db, actor, Some(id.into_inner()), body.into_inner()).await {
        Ok(()) => HttpResponse::Ok().json(serde_json::json!({"saved":true})),
        Err(e) => auth_error(
            if e == "Product not found." {
                actix_web::http::StatusCode::NOT_FOUND
            } else {
                actix_web::http::StatusCode::BAD_REQUEST
            },
            &e,
        ),
    }
}
async fn archive_admin_product(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<uuid::Uuid>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let id = id.into_inner();
    match sqlx::query(
        "UPDATE products SET status='archived',updated_at=now() WHERE id=$1 AND status<>'archived'",
    )
    .bind(id)
    .execute(&state.db)
    .await
    {
        Ok(result) if result.rows_affected() > 0 => {
            audit(
                &state.db,
                actor,
                "product.archived",
                "product",
                &id.to_string(),
            )
            .await;
            HttpResponse::Ok().json(serde_json::json!({"archived":true}))
        }
        Ok(_) => auth_error(actix_web::http::StatusCode::NOT_FOUND, "Product not found."),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not archive product.",
        ),
    }
}

async fn admin_categories(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if let Err(r) = admin_id(&req, &state.db).await {
        return *r;
    }
    match sqlx::query_as::<_, (String, String, bool, i32)>(
        "SELECT name,slug,active,position FROM categories ORDER BY position,name",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => HttpResponse::Ok().json(
            rows.into_iter()
                .map(|r| serde_json::json!({"name":r.0,"slug":r.1,"active":r.2,"position":r.3}))
                .collect::<Vec<_>>(),
        ),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not load categories.",
        ),
    }
}
#[derive(Deserialize)]
struct CategoryInput {
    name: String,
}
async fn create_admin_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    body: web::Json<CategoryInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let name = body.name.trim();
    let slug = slugify(name);
    if name.is_empty() || name.len() > 100 || slug.is_empty() {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Enter a valid category name.",
        );
    }
    match sqlx::query("INSERT INTO categories(name,slug) VALUES($1,$2)")
        .bind(name)
        .bind(&slug)
        .execute(&state.db)
        .await
    {
        Ok(_) => {
            audit(&state.db, actor, "category.created", "category", &slug).await;
            HttpResponse::Created().json(serde_json::json!({"name":name,"slug":slug}))
        }
        Err(error) => {
            if let Some(database_error) = error.as_database_error() {
                if database_error.code().as_deref() == Some("23505") {
                    let message = match database_error.constraint() {
                        Some("categories_name_key") => "A category with this name already exists.",
                        Some("categories_slug_key") => "A category with a similar URL name already exists. Try a more distinct name.",
                        _ => "A category with this name or URL name already exists.",
                    };
                    return auth_error(actix_web::http::StatusCode::CONFLICT, message);
                }
            }
            tracing::error!(error=%error, "category creation failed");
            auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not create category. Check the server logs for details.",
            )
        }
    }
}

async fn remove_admin_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    slug: web::Path<String>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match sqlx::query("UPDATE categories SET active=false WHERE slug=$1 AND active=true")
        .bind(slug.as_str())
        .execute(&state.db)
        .await
    {
        Ok(result) if result.rows_affected() > 0 => {
            audit(&state.db, actor, "category.deactivated", "category", &slug).await;
            HttpResponse::Ok().json(serde_json::json!({"removed":true}))
        }
        Ok(_) => auth_error(actix_web::http::StatusCode::NOT_FOUND, "Active category not found."),
        Err(error) => {
            tracing::error!(error=%error, "category removal failed");
            auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Could not remove category.")
        }
    }
}

#[derive(Deserialize)]
struct CategoryStatusInput {
    active: bool,
}

async fn update_admin_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    slug: web::Path<String>,
    body: web::Json<CategoryStatusInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match sqlx::query("UPDATE categories SET active=$2 WHERE slug=$1 AND active<>$2")
        .bind(slug.as_str())
        .bind(body.active)
        .execute(&state.db)
        .await
    {
        Ok(result) if result.rows_affected() > 0 => {
            audit(&state.db, actor, if body.active { "category.restored" } else { "category.deactivated" }, "category", &slug).await;
            HttpResponse::Ok().json(serde_json::json!({"active":body.active}))
        }
        Ok(_) => auth_error(actix_web::http::StatusCode::NOT_FOUND, "Category not found or already in that state."),
        Err(error) => {
            tracing::error!(error=%error, "category status update failed");
            auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR, "Could not update category.")
        }
    }
}

async fn admin_orders(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if let Err(r) = admin_id(&req, &state.db).await {
        return *r;
    }
    let rows=sqlx::query_as::<_,(String,String,String,String,String,serde_json::Value,String,String,String,String,chrono::DateTime<chrono::Utc>,serde_json::Value)>("SELECT o.id::text,o.order_number,o.status,o.payment_method,o.payment_status,o.shipping_snapshot,u.full_name,u.email,o.subtotal::text,o.total::text,o.created_at,COALESCE((SELECT jsonb_agg(jsonb_build_object('product_name',oi.product_name,'sku',oi.sku,'variant',oi.variant,'quantity',oi.quantity,'unit_price',oi.unit_price::text)) FROM order_items oi WHERE oi.order_id=o.id),'[]'::jsonb) FROM orders o JOIN users u ON u.id=o.user_id ORDER BY o.created_at DESC LIMIT 100").fetch_all(&state.db).await;
    match rows{Ok(rows)=>HttpResponse::Ok().json(rows.into_iter().map(|r|serde_json::json!({"id":r.0,"order_number":r.1,"status":r.2,"payment_method":r.3,"payment_status":r.4,"shipping":r.5,"customer_name":r.6,"customer_email":r.7,"subtotal":r.8,"total":r.9,"created_at":r.10,"items":r.11})).collect::<Vec<_>>()),Err(_)=>auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not load orders.")}
}
#[derive(Deserialize)]
struct OrderStatusInput {
    status: String,
}
async fn update_admin_order(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<uuid::Uuid>,
    body: web::Json<OrderStatusInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let id = id.into_inner();
    let to = body.status.as_str();
    if !matches!(
        to,
        "confirmed" | "processing" | "shipped" | "delivered" | "cancelled" | "refunded"
    ) {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid order status.",
        );
    }
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(_) => {
            return auth_error(
                actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not update order.",
            );
        }
    };
    let from = sqlx::query_scalar::<_, String>("SELECT status FROM orders WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await;
    let Ok(Some(from)) = from else {
        return auth_error(actix_web::http::StatusCode::NOT_FOUND, "Order not found.");
    };
    if !can_transition_order(&from, to) {
        return auth_error(
            actix_web::http::StatusCode::CONFLICT,
            "That order status change is not allowed.",
        );
    }
    if to=="cancelled"&&sqlx::query("UPDATE products p SET stock=p.stock+items.quantity,updated_at=now() FROM (SELECT product_id,SUM(quantity)::integer AS quantity FROM order_items WHERE order_id=$1 AND product_id IS NOT NULL GROUP BY product_id) items WHERE items.product_id=p.id").bind(id).execute(&mut *tx).await.is_err(){return auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not restore inventory.")}
    if sqlx::query("UPDATE orders SET status=$2,payment_status=CASE WHEN $2='refunded' THEN 'refunded' ELSE payment_status END,updated_at=now() WHERE id=$1").bind(id).bind(to).execute(&mut *tx).await.is_err()||tx.commit().await.is_err(){return auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not update order.")}
    audit(
        &state.db,
        actor,
        "order.status_changed",
        "order",
        &id.to_string(),
    )
    .await;
    HttpResponse::Ok().json(serde_json::json!({"updated":true,"status":to}))
}

async fn admin_users(req: HttpRequest, state: web::Data<AppState>) -> HttpResponse {
    if let Err(r) = admin_id(&req, &state.db).await {
        return *r;
    }
    let admin_emails: Vec<String> = std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    match sqlx::query_as::<_,(String,String,String,String,chrono::DateTime<chrono::Utc>)>("SELECT id::text,email,full_name,status,created_at FROM users WHERE role='customer' AND NOT(lower(email)=ANY($1)) ORDER BY created_at DESC LIMIT 200").bind(admin_emails).fetch_all(&state.db).await{Ok(rows)=>HttpResponse::Ok().json(rows.into_iter().map(|r|serde_json::json!({"id":r.0,"email":r.1,"full_name":r.2,"status":r.3,"created_at":r.4})).collect::<Vec<_>>()),Err(_)=>auth_error(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,"Could not load customers.")}
}
#[derive(Deserialize)]
struct UserStatusInput {
    status: String,
}
async fn update_admin_user(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: web::Path<uuid::Uuid>,
    body: web::Json<UserStatusInput>,
) -> HttpResponse {
    let actor = match admin_id(&req, &state.db).await {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let id = id.into_inner();
    if !matches!(body.status.as_str(), "active" | "blocked" | "disabled") {
        return auth_error(
            actix_web::http::StatusCode::BAD_REQUEST,
            "Invalid customer status.",
        );
    }
    if actor == id {
        return auth_error(
            actix_web::http::StatusCode::CONFLICT,
            "You cannot change your own account status.",
        );
    }
    let target = sqlx::query_as::<_, (String, String)>("SELECT email,role FROM users WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.db)
        .await;
    let Ok(Some((target_email, target_role))) = target else {
        return auth_error(
            actix_web::http::StatusCode::NOT_FOUND,
            "Customer not found.",
        );
    };
    if target_role != "customer" || is_admin(&target_email) {
        return auth_error(
            actix_web::http::StatusCode::FORBIDDEN,
            "Administrator accounts cannot be changed here.",
        );
    }
    let result =
        sqlx::query("UPDATE users SET status=$2,updated_at=now() WHERE id=$1 AND role='customer'")
            .bind(id)
            .bind(&body.status)
            .execute(&state.db)
            .await;
    match result {
        Ok(r) if r.rows_affected() > 0 => {
            if body.status != "active" {
                let _=sqlx::query("UPDATE user_sessions SET revoked_at=now() WHERE user_id=$1 AND revoked_at IS NULL").bind(id).execute(&state.db).await;
            }
            audit(
                &state.db,
                actor,
                "customer.status_changed",
                "user",
                &id.to_string(),
            )
            .await;
            HttpResponse::Ok().json(serde_json::json!({"updated":true,"status":body.status}))
        }
        Ok(_) => auth_error(
            actix_web::http::StatusCode::NOT_FOUND,
            "Customer not found.",
        ),
        Err(_) => auth_error(
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Could not update customer.",
        ),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "backend=info,actix_web=info".into()),
        )
        .init();
    let database_url = env::var("DATABASE_URL").map_err(|_| {
        std::io::Error::other(
            "DATABASE_URL is required; copy backend/.env.example and configure Supabase PostgreSQL",
        )
    })?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await
        .map_err(|e| std::io::Error::other(format!("database connection failed: {e}")))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| std::io::Error::other(format!("database migration failed: {e}")))?;
    let origins: Vec<String> = env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173".into())
        .split(',')
        .map(|x| x.trim().to_owned())
        .filter(|x| !x.is_empty())
        .collect();
    let bind = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let state = web::Data::new(AppState { db: pool });
    HttpServer::new(move || {
        let cors = origins
            .iter()
            .fold(Cors::default(), |cors, origin| cors.allowed_origin(origin))
            .allowed_methods(vec!["GET", "POST", "PATCH", "DELETE", "OPTIONS"])
            .allowed_header(actix_web::http::header::CONTENT_TYPE)
            .supports_credentials()
            .max_age(3600);
        App::new()
            .app_data(state.clone())
            .wrap(cors)
            .wrap(
                middleware::DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("Referrer-Policy", "no-referrer")),
            )
            .wrap(middleware::Logger::default())
            .route("/health", web::get().to(health))
            .route("/api/v1/products", web::get().to(products))
            .route("/api/v1/products/{slug}", web::get().to(product))
            .route("/api/v1/categories", web::get().to(categories))
            .route("/api/v1/auth/register", web::post().to(register))
            .route("/api/v1/auth/verify-email", web::post().to(verify_email))
            .route("/api/v1/auth/login", web::post().to(login))
            .route("/api/v1/auth/google", web::post().to(google_login))
            .route("/api/v1/auth/me", web::get().to(me))
            .route("/api/v1/auth/logout", web::post().to(logout))
            .route("/api/v1/checkout", web::post().to(checkout))
            .route("/api/v1/orders", web::get().to(customer_orders))
            .route("/api/v1/admin/dashboard", web::get().to(admin_dashboard))
            .route("/api/v1/admin/products", web::get().to(admin_products))
            .route(
                "/api/v1/admin/products",
                web::post().to(create_admin_product),
            )
            .route(
                "/api/v1/admin/products/{id}",
                web::patch().to(update_admin_product),
            )
            .route(
                "/api/v1/admin/products/{id}",
                web::delete().to(archive_admin_product),
            )
            .route("/api/v1/admin/categories", web::get().to(admin_categories))
            .route(
                "/api/v1/admin/categories",
                web::post().to(create_admin_category),
            )
            .route("/api/v1/admin/categories/{slug}", web::delete().to(remove_admin_category))
            .route("/api/v1/admin/categories/{slug}", web::patch().to(update_admin_category))
            .route("/api/v1/admin/orders", web::get().to(admin_orders))
            .route(
                "/api/v1/admin/orders/{id}",
                web::patch().to(update_admin_order),
            )
            .route("/api/v1/admin/users", web::get().to(admin_users))
            .route(
                "/api/v1/admin/users/{id}",
                web::patch().to(update_admin_user),
            )
    })
    .bind(bind)?
    .run()
    .await
}

#[cfg(test)]
mod tests {
    use super::{can_transition_order, normalize_email, slugify};

    #[test]
    fn normalizes_and_rejects_disposable_addresses() {
        assert_eq!(
            normalize_email("  Shop@Example.com ").as_deref(),
            Some("shop@example.com")
        );
        assert!(normalize_email("not-an-email").is_none());
        assert!(normalize_email("person@mailinator.com").is_none());
    }

    #[test]
    fn category_slug_is_stable() {
        assert_eq!(slugify("Hoodies & Sweatshirts"), "hoodies-sweatshirts");
    }

    #[test]
    fn order_statuses_only_move_forward_or_cancel() {
        assert!(can_transition_order("pending", "confirmed"));
        assert!(can_transition_order("processing", "cancelled"));
        assert!(!can_transition_order("delivered", "processing"));
        assert!(!can_transition_order("cancelled", "confirmed"));
    }
}
