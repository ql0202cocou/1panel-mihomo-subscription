//! 全局自定义节点池(跨订阅共享)验收测试:列表 / 更新 / 删除。

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, Response, StatusCode};
use axum::Router;
use mihomo_subscription::app::build_router;
use mihomo_subscription::fetch::{FetchError, Fetched, SubscriptionFetcher};
use serde_json::Value;
use tower::util::ServiceExt;

use common::{test_state_with_fetcher, TempDb};

#[derive(Clone, Default)]
struct FakeFetcher;

#[async_trait::async_trait]
impl SubscriptionFetcher for FakeFetcher {
    async fn fetch(&self, _url: &str) -> Result<Fetched, FetchError> {
        Ok(Fetched {
            body: "proxies: []\nrules:\n  - MATCH,DIRECT\n".to_string(),
            subscription_userinfo: None,
        })
    }
}

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

fn authed(method: &str, path: &str, cookie: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::HOST, "sub.example.com")
        .header(header::ORIGIN, "https://sub.example.com")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json(resp: Response<Body>) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

const NODE_CONTENT: &str =
    "{ type: ss, server: 1.2.3.4, port: 8388, cipher: aes-128-gcm, password: test }";

async fn create_node(app: &Router, cookie: &str, name: &str) -> Value {
    let body = format!(r#"{{"name":"{name}","node_type":"ss","content":"{NODE_CONTENT}"}}"#);
    let resp = app
        .clone()
        .oneshot(authed("POST", "/api/global-nodes", cookie, &body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    json(resp).await
}

#[tokio::test]
async fn list_returns_nodes_in_block_order() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher)).await);
    let cookie = login(&app).await;

    // 空池返回空数组。
    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/global-nodes", &cookie, ""))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(json(resp).await.as_array().unwrap().len(), 0);

    create_node(&app, &cookie, "hk-1").await;
    create_node(&app, &cookie, "jp-1").await;

    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/global-nodes", &cookie, ""))
        .await
        .unwrap();
    let nodes = json(resp).await;
    let names: Vec<&str> = nodes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["hk-1", "jp-1"]);
    assert!(nodes[0]["enabled"].as_bool().unwrap());
}

#[tokio::test]
async fn update_and_delete_by_id() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher)).await);
    let cookie = login(&app).await;

    let created = create_node(&app, &cookie, "hk-1").await;
    let id = created["id"].as_str().unwrap();

    // 更新:改名 + 停用,响应带最新字段。
    let body =
        format!(r#"{{"name":"hk-2","node_type":"ss","content":"{NODE_CONTENT}","enabled":false}}"#);
    let resp = app
        .clone()
        .oneshot(authed(
            "PUT",
            &format!("/api/global-nodes/{id}"),
            &cookie,
            &body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let updated = json(resp).await;
    assert_eq!(updated["name"].as_str().unwrap(), "hk-2");
    assert!(!updated["enabled"].as_bool().unwrap());

    // 非法 content(不是 YAML 映射)→ 400。
    let resp = app
        .clone()
        .oneshot(authed(
            "PUT",
            &format!("/api/global-nodes/{id}"),
            &cookie,
            r#"{"name":"hk-2","node_type":"ss","content":"- just\n- a\n- list"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 更新不存在的 id → 404。
    let resp = app
        .clone()
        .oneshot(authed(
            "PUT",
            "/api/global-nodes/does-not-exist",
            &cookie,
            &body,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 删除 → 204,列表为空;再删 → 404。
    let resp = app
        .clone()
        .oneshot(authed(
            "DELETE",
            &format!("/api/global-nodes/{id}"),
            &cookie,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/global-nodes", &cookie, ""))
        .await
        .unwrap();
    assert_eq!(json(resp).await.as_array().unwrap().len(), 0);
    let resp = app
        .clone()
        .oneshot(authed(
            "DELETE",
            &format!("/api/global-nodes/{id}"),
            &cookie,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
