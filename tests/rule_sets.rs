//! 全局规则集库(「规则托管」,② 用户库 / 导入源)验收测试。
//!
//! 0.4 起 ② 仅作模板/导入源:不再公开托管、不再参与生成,故本文件只覆盖 CRUD 与计数/脱敏;托管与
//! 注入到订阅的行为见 `tests/profile_rule_sets.rs`(③)。

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

const PROVIDER_YAML: &str =
    "proxies:\n  - { name: hk-1, type: ss, server: 1.2.3.4, port: 8388 }\nrules:\n  - MATCH,DIRECT\n";

#[derive(Clone, Default)]
struct FakeFetcher;

#[async_trait::async_trait]
impl SubscriptionFetcher for FakeFetcher {
    async fn fetch(&self, _url: &str) -> Result<Fetched, FetchError> {
        Ok(Fetched {
            body: PROVIDER_YAML.to_string(),
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

#[tokio::test]
async fn crud_counts_and_masks_no_hosted_link() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher)).await);
    let cookie = login(&app).await;

    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/rule-sets",
            &cookie,
            r#"{"name":"adblock","behavior":"domain","format":"yaml","content":"+.doubleclick.net\n# comment\n+.googleads.com"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = json(resp).await;
    // count 只数有效 payload 行;② 不再有托管链接字段。
    assert_eq!(created["count"].as_u64().unwrap(), 2);
    assert!(created.get("url").is_none(), "② 不再暴露托管链接");
    let id = created["id"].as_str().unwrap().to_string();

    // 列表能看到它。
    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/rule-sets", &cookie, ""))
        .await
        .unwrap();
    assert_eq!(json(resp).await.as_array().unwrap().len(), 1);

    // 重名 → 409;非法 name → 400。
    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/rule-sets",
            &cookie,
            r#"{"name":"adblock","behavior":"ipcidr","format":"text"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/rule-sets",
            &cookie,
            r#"{"name":"a/b","behavior":"domain","format":"yaml"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 删除 → 204,再删 → 404。
    let resp = app
        .clone()
        .oneshot(authed(
            "DELETE",
            &format!("/api/rule-sets/{id}"),
            &cookie,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .clone()
        .oneshot(authed(
            "DELETE",
            &format!("/api/rule-sets/{id}"),
            &cookie,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn set_order_reorders_and_appends_unlisted() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher)).await);
    let cookie = login(&app).await;

    // 建三个规则集:初始顺序 a, b, c。
    for name in ["a-set", "b-set", "c-set"] {
        let body = format!(
            r#"{{"name":"{name}","behavior":"domain","format":"yaml","content":"+.example.com"}}"#
        );
        let resp = app
            .clone()
            .oneshot(authed("POST", "/api/rule-sets", &cookie, &body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // 只列出 c-set(外加一个库中不存在的名字,应被忽略):c-set 提到最前,未列出的保持原有
    // 相对顺序落在末尾。
    let resp = app
        .clone()
        .oneshot(authed(
            "PUT",
            "/api/rule-sets/order",
            &cookie,
            r#"{"order":["c-set","ghost"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/rule-sets", &cookie, ""))
        .await
        .unwrap();
    let list = json(resp).await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["c-set", "a-set", "b-set"]);

    // 再排一次全量顺序,确认批量更新在一个事务内整体生效。
    let resp = app
        .clone()
        .oneshot(authed(
            "PUT",
            "/api/rule-sets/order",
            &cookie,
            r#"{"order":["b-set","a-set","c-set"]}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/rule-sets", &cookie, ""))
        .await
        .unwrap();
    let list = json(resp).await;
    let names: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["b-set", "a-set", "c-set"]);
}

#[tokio::test]
async fn remote_source_masks_url() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher)).await);
    let cookie = login(&app).await;

    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/rule-sets",
            &cookie,
            r#"{"name":"mirror","behavior":"classical","format":"text","source":"remote","url":"https://up.example/list.txt?token=secret"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = json(resp).await;
    assert_eq!(created["source"].as_str().unwrap(), "remote");
    assert!(created["remote_url_masked"]
        .as_str()
        .unwrap()
        .contains("token=***"));

    // ② 不再公开托管:旧的全局 /r/ 路由已移除,落到 SPA 兜底(非规则内容)。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/mirror/classical.text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    assert!(
        !String::from_utf8_lossy(&bytes).contains("MATCH,DIRECT"),
        "② 全局库不再公开托管规则内容"
    );
}
