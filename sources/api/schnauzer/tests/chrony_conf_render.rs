//! Render tests for the chrony-conf template.
//!
//! `settings.ntp.time-servers` can be a URL list or a per-server representation.
//! These tests render each supported representation with schnauzer's real helpers.

use schnauzer::v2::import::{JsonSettingsResolver, StaticHelperResolver, TemplateImporter};
use serde_json::json;

const CHRONY_CONF: &str = include_str!("../../../../packages/chrony/chrony-conf");

struct TestImporter {
    settings_resolver: JsonSettingsResolver,
    helper_resolver: StaticHelperResolver,
}

impl TemplateImporter for TestImporter {
    type SettingsResolver = JsonSettingsResolver;
    type HelperResolver = StaticHelperResolver;

    fn settings_resolver(&self) -> &Self::SettingsResolver {
        &self.settings_resolver
    }

    fn helper_resolver(&self) -> &Self::HelperResolver {
        &self.helper_resolver
    }
}

async fn render(settings: serde_json::Value) -> String {
    let importer = TestImporter {
        settings_resolver: JsonSettingsResolver::new(settings),
        helper_resolver: StaticHelperResolver,
    };
    schnauzer::v2::render_template_str(&importer, CHRONY_CONF)
        .await
        .expect("chrony-conf should render without error")
}

fn assert_no_equals_in_directives(rendered: &str) {
    for line in rendered.lines() {
        assert!(
            !line.contains('='),
            "unexpected '=' in rendered chrony.conf line: {line:?}"
        );
    }
}

fn assert_no_blank_lines(rendered: &str) {
    for (i, line) in rendered.lines().enumerate() {
        assert!(
            !line.trim().is_empty(),
            "unexpected blank line at line {} in:\n{}",
            i + 1,
            rendered
        );
    }
}

#[tokio::test]
async fn named_branch_renders() {
    let rendered = render(json!({
        "settings": {
            "ntp": {
                "logging": ["measurements", "statistics", "tracking"],
                "time-servers": {
                    "link-local": {
                        "address": "169.254.169.123",
                        "directive": "server",
                        "options": ["iburst", "prefer", "minpoll 4", "maxpoll 4"]
                    },
                    "amazon-pool": {
                        "address": "time.aws.com",
                        "directive": "pool",
                        "options": ["iburst"]
                    }
                }
            }
        }
    }))
    .await;

    assert!(
        rendered.contains("server 169.254.169.123 iburst prefer minpoll 4 maxpoll 4"),
        "link-local server line missing/wrong:\n{rendered}"
    );
    assert!(
        rendered.contains("pool time.aws.com iburst"),
        "amazon-pool line missing/wrong:\n{rendered}"
    );
    assert!(
        rendered.contains("log measurements statistics tracking"),
        "logging line missing/wrong:\n{rendered}"
    );
    assert!(rendered.contains("driftfile /var/lib/chrony/drift"));
    assert!(rendered.contains("logdir /var/log/chrony"));
    assert!(rendered.trim_end().ends_with("rtcsync"));

    assert_no_equals_in_directives(&rendered);
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn named_partial_server_renders() {
    // Optional fields are omitted from the JSON, so the template must guard them.
    let rendered = render(json!({
        "settings": {
            "ntp": {
                "time-servers": {
                    "no-directive": {
                        "address": "169.254.169.123",
                        "options": ["iburst"]
                    },
                    "no-options": {
                        "address": "time.aws.com",
                        "directive": "pool"
                    },
                    "bare": {
                        "address": "time2.aws.com"
                    }
                }
            }
        }
    }))
    .await;

    assert!(
        rendered.contains("server 169.254.169.123 iburst"),
        "no-directive server should default to `server`:\n{rendered}"
    );
    assert!(
        rendered.contains("pool time.aws.com"),
        "no-options server line missing/wrong:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|l| l == "pool time.aws.com"),
        "no-options line should have no trailing whitespace/options:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|l| l == "server time2.aws.com"),
        "bare server should render `server time2.aws.com`:\n{rendered}"
    );

    assert!(rendered.contains("driftfile /var/lib/chrony/drift"));
    assert!(rendered.trim_end().ends_with("rtcsync"));
    assert_no_equals_in_directives(&rendered);
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn named_missing_address_is_skipped() {
    let rendered = render(json!({
        "settings": {
            "ntp": {
                "time-servers": {
                    "broken": {
                        "directive": "pool",
                        "options": ["iburst"]
                    },
                    "good": {
                        "address": "time.aws.com",
                        "directive": "server",
                        "options": ["iburst"]
                    }
                }
            }
        }
    }))
    .await;

    assert!(
        rendered.contains("server time.aws.com iburst"),
        "good server line missing/wrong:\n{rendered}"
    );
    assert!(
        !rendered
            .lines()
            .any(|l| l.trim() == "pool" || l.starts_with("pool ") && !l.contains("time.aws.com")),
        "a server with no address should be skipped, not render a bare directive:\n{rendered}"
    );
    assert_no_equals_in_directives(&rendered);
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn legacy_branch_renders() {
    let rendered = render(json!({
        "settings": {
            "ntp": {
                "logging": ["measurements"],
                "options": ["iburst"],
                "time-servers": ["https://time.aws.com", "https://time2.aws.com"]
            }
        }
    }))
    .await;

    assert!(
        rendered.contains("pool https://time.aws.com iburst"),
        "first pool line missing/wrong:\n{rendered}"
    );
    assert!(
        rendered.contains("pool https://time2.aws.com iburst"),
        "second pool line missing/wrong:\n{rendered}"
    );
    assert!(
        !rendered.contains("server "),
        "legacy branch unexpectedly rendered a `server` directive:\n{rendered}"
    );
    assert!(rendered.contains("log measurements"));

    assert_no_equals_in_directives(&rendered);
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn object_list_renders() {
    let rendered = render(json!({"settings":{"ntp":{"time-servers":[
        {"address":"169.254.169.123","directive":"server","options":["prefer","iburst","minpoll 4","maxpoll 4"]},
        {"address":"time.aws.com","directive":"pool","options":["iburst"]}
    ]}}})).await;

    assert!(rendered.contains("server 169.254.169.123 prefer iburst minpoll 4 maxpoll 4"));
    assert!(rendered.contains("pool time.aws.com iburst"));
    assert_no_equals_in_directives(&rendered);
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn object_list_uses_its_own_options_and_default_directive() {
    let rendered = render(json!({"settings":{"ntp":{
        "options":["iburst","prefer"],
        "time-servers":[
            {"address":"a.example"},
            {"address":"b.example","directive":"pool","options":[]}
        ]
    }}}))
    .await;

    assert!(rendered.lines().any(|line| line == "server a.example"));
    assert!(rendered.lines().any(|line| line == "pool b.example"));
    assert!(!rendered.contains("iburst"));
    assert!(!rendered.contains("prefer"));
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn legacy_list_with_shared_options_renders() {
    let rendered = render(
        json!({"settings":{"ntp":{"time-servers":["a.example","b.example"],"options":["iburst"]}}}),
    )
    .await;

    assert!(rendered.contains("pool a.example iburst"));
    assert!(rendered.contains("pool b.example iburst"));
    assert!(!rendered.contains("server "));
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn empty_list_renders_no_servers() {
    let rendered = render(json!({"settings":{"ntp":{"time-servers":[]}}})).await;

    assert!(!rendered
        .lines()
        .any(|line| line.starts_with("server ") || line.starts_with("pool ")));
    assert_no_blank_lines(&rendered);
}

#[tokio::test]
async fn named_map_still_renders() {
    let rendered = render(json!({"settings":{"ntp":{"time-servers":{"x":{"address":"time.aws.com","directive":"pool","options":["iburst"]}}}}})).await;

    assert!(rendered.contains("pool time.aws.com iburst"));
    assert_no_blank_lines(&rendered);
}
