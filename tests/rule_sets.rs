//! 全局规则集托管(「规则配置」)验收测试:CRUD + 托管链接元数据、公开 `/r/` 端点的
//! yaml/text 渲染与统一 404、以及被 `RULE-SET` 规则引用的规则集注入到输出 `rule-providers:`
//! (未被引用的不注入)。用 fake fetcher,避免真实网络/SSRF。

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

/// 固定 body 的 fetcher;默认回 `PROVIDER_YAML`,撞名测试传 `PROVIDER_WITH_RULE_PROVIDERS`。
#[derive(Clone)]
struct FakeFetcher {
    body: &'static str,
}

impl Default for FakeFetcher {
    fn default() -> Self {
        Self {
            body: PROVIDER_YAML,
        }
    }
}

#[async_trait::async_trait]
impl SubscriptionFetcher for FakeFetcher {
    async fn fetch(&self, _url: &str) -> Result<Fetched, FetchError> {
        Ok(Fetched {
            body: self.body.to_string(),
            subscription_userinfo: None,
        })
    }
}

/// 机场自带一个名为 `ads` 的 `rule-provider`,用于撞名告警测试。
const PROVIDER_WITH_RULE_PROVIDERS: &str = "proxies:\n  - { name: hk-1, type: ss, server: 1.2.3.4, port: 8388 }\nrule-providers:\n  ads: { type: http, behavior: domain, url: https://up.example/ads.yaml }\nrules:\n  - MATCH,DIRECT\n";

async fn login(app: &Router) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::post("/api/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
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

async fn text(resp: Response<Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

async fn create_rule_set(app: &Router, cookie: &str, body: &str) -> Value {
    let resp = app
        .clone()
        .oneshot(authed("POST", "/api/rule-sets", cookie, body))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    json(resp).await
}

#[tokio::test]
async fn crud_exposes_hosted_link_and_count() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher::default())).await);
    let cookie = login(&app).await;

    let created = create_rule_set(
        &app,
        &cookie,
        r#"{"name":"adblock","behavior":"domain","format":"yaml","content":"+.doubleclick.net\n# comment\n+.googleads.com"}"#,
    )
    .await;
    // 托管链接按 名/behavior.format 拼装;count 只数有效 payload 行(忽略空行与注释)。
    assert_eq!(
        created["url"].as_str().unwrap(),
        "https://sub.example.com/testprefix/r/adblock/domain.yaml"
    );
    assert_eq!(created["count"].as_u64().unwrap(), 2);
    let id = created["id"].as_str().unwrap().to_string();

    // 列表能看到它。
    let resp = app
        .clone()
        .oneshot(authed("GET", "/api/rule-sets", &cookie, ""))
        .await
        .unwrap();
    let list = json(resp).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // 重名 → 409。
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

    // 非法 name(含 '/')→ 400。
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
async fn public_serve_renders_and_404s() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher::default())).await);
    let cookie = login(&app).await;

    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"ads","behavior":"domain","format":"yaml","content":"+.doubleclick.net"}"#,
    )
    .await;
    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"direct","behavior":"classical","format":"text","content":"DOMAIN-SUFFIX,lan"}"#,
    )
    .await;

    // yaml → payload 列表。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/ads/domain.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = text(resp).await;
    assert!(body.contains("payload:"), "yaml 渲染为 payload 列表");
    assert!(body.contains("+.doubleclick.net"));

    // text → 逐行原样。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/direct/classical.text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = text(resp).await;
    assert!(!body.contains("payload:"), "text 不包 payload");
    assert!(body.contains("DOMAIN-SUFFIX,lan"));

    // 文件名(behavior.format)不符 → 404。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/ads/domain.text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 前缀错 → 404。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/wrongprefix/r/ads/domain.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // 名不存在 → 404。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/nope/domain.yaml")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn referenced_rule_set_injected_only_when_used() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher::default())).await);
    let cookie = login(&app).await;

    // 建 profile。
    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/profiles",
            &cookie,
            r#"{"name":"P","source_url":"https://provider.example/sub"}"#,
        ))
        .await
        .unwrap();
    let profile = json(resp).await;
    let id = profile["id"].as_str().unwrap().to_string();
    let sub = profile["subscription_url"].as_str().unwrap();
    let sub_path = {
        let after = sub.split("://").nth(1).unwrap();
        after[after.find('/').unwrap()..].to_string()
    };

    // 规则引用规则集 `ads`(但不引用 `extra`)。
    app.clone()
        .oneshot(authed(
            "PUT",
            &format!("/api/profiles/{id}/rules"),
            &cookie,
            r#"{"content":"RULE-SET,ads,DIRECT\nMATCH,DIRECT"}"#,
        ))
        .await
        .unwrap();

    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"ads","behavior":"domain","format":"yaml","content":"+.ad.example"}"#,
    )
    .await;
    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"extra","behavior":"ipcidr","format":"text","content":"10.0.0.0/8"}"#,
    )
    .await;

    // 拉公开订阅:被引用的 `ads` 注入到 rule-providers,指向托管链接;未引用的 `extra` 不注入。
    let resp = app
        .clone()
        .oneshot(Request::get(&sub_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = text(resp).await;
    assert!(body.contains("rule-providers:"), "应注入 rule-providers");
    assert!(
        body.contains("https://sub.example.com/testprefix/r/ads/domain.yaml"),
        "ads 指向托管链接"
    );
    assert!(body.contains("behavior: domain"));
    assert!(
        !body.contains("/r/extra/"),
        "未被 RULE-SET 引用的规则集不应注入"
    );
}

#[tokio::test]
async fn rule_set_name_colliding_with_provider_is_reported() {
    let temp = TempDb::new();
    let app = build_router(
        test_state_with_fetcher(
            &temp,
            Arc::new(FakeFetcher {
                body: PROVIDER_WITH_RULE_PROVIDERS,
            }),
        )
        .await,
    );
    let cookie = login(&app).await;

    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/profiles",
            &cookie,
            r#"{"name":"P","source_url":"https://provider.example/sub"}"#,
        ))
        .await
        .unwrap();
    let id = json(resp).await["id"].as_str().unwrap().to_string();

    // 引用 `ads`,并建一个同名自定义规则集 `ads`(与机场 rule-provider 撞名)。
    app.clone()
        .oneshot(authed(
            "PUT",
            &format!("/api/profiles/{id}/rules"),
            &cookie,
            r#"{"content":"RULE-SET,ads,DIRECT\nMATCH,DIRECT"}"#,
        ))
        .await
        .unwrap();
    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"ads","behavior":"domain","format":"yaml","content":"+.ad.example"}"#,
    )
    .await;

    // 生成响应应在 `ruleset_conflicts` 里报告 `ads` 撞名(覆盖语义不变,但不再静默)。
    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            &format!("/api/profiles/{id}/generate"),
            &cookie,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json(resp).await;
    let conflicts: Vec<&str> = body["ruleset_conflicts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(conflicts, vec!["ads"], "应报告与机场撞名的规则集");
}

#[tokio::test]
async fn remote_mirror_serves_fetched_bytes() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher::default())).await);
    let cookie = login(&app).await;

    // 远程来源 + 本地缓存托管:面板拉取上游并以稳定链接二次托管。
    let created = create_rule_set(
        &app,
        &cookie,
        r#"{"name":"mirror","behavior":"classical","format":"text","source":"remote","url":"https://up.example/list.txt?token=secret"}"#,
    )
    .await;
    assert_eq!(created["source"].as_str().unwrap(), "remote");
    // 远程 URL 在响应里脱敏(查询值 -> ***)。
    assert!(created["remote_url_masked"]
        .as_str()
        .unwrap()
        .contains("token=***"));

    // 拉托管链接 → 返回上游内容(FakeFetcher 回的 body 原样)。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/mirror/classical.text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = text(resp).await;
    assert!(body.contains("MATCH,DIRECT"), "镜像并托管了上游字节");
}

#[tokio::test]
async fn remote_cache_off_injects_upstream_url() {
    let temp = TempDb::new();
    let app = build_router(test_state_with_fetcher(&temp, Arc::new(FakeFetcher::default())).await);
    let cookie = login(&app).await;

    let resp = app
        .clone()
        .oneshot(authed(
            "POST",
            "/api/profiles",
            &cookie,
            r#"{"name":"P","source_url":"https://provider.example/sub"}"#,
        ))
        .await
        .unwrap();
    let profile = json(resp).await;
    let id = profile["id"].as_str().unwrap().to_string();
    let sub = profile["subscription_url"].as_str().unwrap();
    let sub_path = {
        let after = sub.split("://").nth(1).unwrap();
        after[after.find('/').unwrap()..].to_string()
    };

    app.clone()
        .oneshot(authed(
            "PUT",
            &format!("/api/profiles/{id}/rules"),
            &cookie,
            r#"{"content":"RULE-SET,direct,DIRECT\nMATCH,DIRECT"}"#,
        ))
        .await
        .unwrap();

    // 远程 + 关闭本地缓存托管:转换时直接注入上游 URL,/r/ 不托管。
    create_rule_set(
        &app,
        &cookie,
        r#"{"name":"direct","behavior":"classical","format":"text","source":"remote","cache":false,"url":"https://up.example/direct.txt"}"#,
    )
    .await;

    let resp = app
        .clone()
        .oneshot(Request::get(&sub_path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = text(resp).await;
    assert!(
        body.contains("https://up.example/direct.txt"),
        "cache 关闭时直接注入上游 URL"
    );
    assert!(
        !body.contains("/r/direct/"),
        "cache 关闭时不指向面板托管链接"
    );

    // 且托管端点对 cache 关闭的远程集 404(不托管)。
    let resp = app
        .clone()
        .oneshot(
            Request::get("/testprefix/r/direct/classical.text")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
