//! 应用设置端点验收测试:`GET /api/settings`。

mod common;

use axum::body::Body;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use mihomo_subscription::app::build_router;
use serde_json::Value;
use tower::util::ServiceExt;

use common::{test_state, TempDb};

async fn login(app: &Router) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::post("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::HOST, "sub.example.com")
                .header(header::ORIGIN, "https://sub.example.com")
                .body(Body::from(r#"{"username":"admin","password":"s3cret"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    resp.headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn json(resp: Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn get_settings_returns_public_path_prefix() {
    let temp = TempDb::new();
    let app = build_router(test_state(&temp).await);

    // 未认证 → 401。
    let resp = app
        .clone()
        .oneshot(Request::get("/api/settings").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let cookie = login(&app).await;
    let resp = app
        .clone()
        .oneshot(
            Request::get("/api/settings")
                .header(header::COOKIE, &cookie)
                .header(header::HOST, "sub.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // 测试状态固定前缀(见 tests/common/mod.rs)。
    let body = json(resp).await;
    assert_eq!(body["public_path_prefix"].as_str().unwrap(), "testprefix");
}
