use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use flockfly::api::{Api, ApiFactory, CliError};
use flockfly::commands::{run_cli_with, Runtime};
use serde_json::{json, Value};
use tempfile::TempDir;

const PUBLIC_COLLECTION_ID: &str = "coll_public";

#[derive(Default)]
struct Backend {
    approved_email: Option<String>,
    token_email: HashMap<String, String>,
    skills: Vec<Skill>,
    next_skill: usize,
    search_results: Option<Vec<Value>>,
    search_error: Option<CliError>,
    load_error: Option<CliError>,
    load_requests: Vec<String>,
}

#[derive(Clone)]
struct Skill {
    id: String,
    name: String,
    description: String,
    version: u64,
    files: HashMap<String, String>,
    attached: bool,
}

#[derive(Clone)]
struct FakeFactory(Arc<Mutex<Backend>>);

struct FakeApi {
    state: Arc<Mutex<Backend>>,
    token: Option<String>,
}

impl ApiFactory for FakeFactory {
    fn create(&self, _base_url: &str, token: Option<&str>) -> Box<dyn Api> {
        Box::new(FakeApi {
            state: Arc::clone(&self.0),
            token: token.map(str::to_owned),
        })
    }
}

impl FakeApi {
    fn email(&self, state: &Backend) -> Result<String, CliError> {
        self.token
            .as_ref()
            .and_then(|token| state.token_email.get(token))
            .cloned()
            .ok_or_else(|| CliError::new("Not authenticated.", Some("unauthenticated"), None))
    }

    fn personal_router(email: &str) -> Value {
        let username = email.split('@').next().unwrap_or("user");
        json!({
            "id": format!("router_{username}"),
            "homeOrgId": format!("org_{username}"),
            "name": format!("{username}'s router"),
            "createdByUserId": format!("user_{username}"),
            "createdAt": "2026-07-28T00:00:00Z"
        })
    }

    fn public_collection() -> Value {
        json!({
            "id": PUBLIC_COLLECTION_ID,
            "kind": "public",
            "orgId": Value::Null,
            "name": "Public",
            "description": Value::Null,
            "createdByUserId": "user_system",
            "createdAt": "2026-07-28T00:00:00Z"
        })
    }
}

fn paged(skills: Vec<Value>) -> Value {
    json!({
        "skills": skills,
        "page": { "limit": 100, "hasMore": false, "nextCursor": Value::Null }
    })
}

impl Api for FakeApi {
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value, CliError> {
        let mut state = self.state.lock().unwrap();

        match (method, path) {
            ("POST", "/v1/auth/cli/start") => Ok(json!({
                "cliAuthId": "auth_abc",
                "verificationUrl": "http://browser.test/login#cliAuthId=auth_abc"
            })),
            ("POST", "/v1/auth/cli/poll") => {
                if let Some(email) = state.approved_email.clone() {
                    let token = format!("ffly_{}", email.replace(['@', '.'], "_"));
                    state.token_email.insert(token.clone(), email);
                    Ok(json!({ "status": "approved", "token": token }))
                } else {
                    Ok(json!({ "status": "pending" }))
                }
            }
            ("GET", "/v1/me") => {
                let email = self.email(&state)?;
                let username = email.split('@').next().unwrap_or("user");
                Ok(json!({
                    "user": {
                        "id": format!("user_{username}"),
                        "email": email,
                        "username": username,
                        "createdAt": "2026-07-28T00:00:00Z"
                    },
                    "org": {
                        "id": format!("org_{username}"),
                        "name": format!("{username}'s org"),
                        "createdAt": "2026-07-28T00:00:00Z"
                    },
                    "personalRouter": Self::personal_router(&email)
                }))
            }
            ("GET", "/v1/collections") => {
                let _email = self.email(&state)?;
                Ok(json!({ "collections": [Self::public_collection()] }))
            }
            ("GET", "/v1/routers") => {
                let email = self.email(&state)?;
                Ok(json!({ "routers": [Self::personal_router(&email)] }))
            }
            ("POST", path) if path.starts_with("/v1/collections/") && path.ends_with("/skills") => {
                let _email = self.email(&state)?;
                let body = body.unwrap();
                let files = body["files"].as_array().unwrap();
                let decoded: HashMap<String, String> = files
                    .iter()
                    .map(|file| {
                        let path = file["path"].as_str().unwrap().to_owned();
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(file["contentBase64"].as_str().unwrap())
                            .unwrap();
                        (path, String::from_utf8(bytes).unwrap())
                    })
                    .collect();
                let skill_md = decoded.get("SKILL.md").unwrap();
                let yaml = skill_md.split("---").nth(1).unwrap();
                let frontmatter: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
                let name = frontmatter["name"].as_str().unwrap().to_owned();
                let description = frontmatter["description"].as_str().unwrap().to_owned();

                if let Some(existing) = state.skills.iter_mut().find(|skill| skill.name == name) {
                    let confirmed = body["confirmReplace"].as_bool() == Some(true)
                        && body["replaceSkillId"].as_str() == Some(existing.id.as_str());
                    if !confirmed {
                        return Err(CliError::new(
                            "A skill with this name already exists.",
                            Some("skill_name_conflict"),
                            Some(json!({
                                "existingSkillId": existing.id,
                                "nextVersion": existing.version + 1
                            })),
                        ));
                    }
                    existing.version += 1;
                    existing.description = description;
                    existing.files = decoded;
                    if body.get("routerId").and_then(Value::as_str).is_some() {
                        existing.attached = true;
                    }
                    return Ok(publish_response(existing));
                }

                state.next_skill += 1;
                let skill = Skill {
                    id: format!("skill_{}", state.next_skill),
                    name,
                    description,
                    version: 1,
                    files: decoded,
                    attached: body.get("routerId").and_then(Value::as_str).is_some(),
                };
                let response = publish_response(&skill);
                state.skills.push(skill);
                Ok(response)
            }
            ("POST", path) if path.starts_with("/v1/routers/") && path.contains("/skills/") => {
                let _email = self.email(&state)?;
                let skill_id = path.rsplit('/').next().unwrap();
                let skill = state
                    .skills
                    .iter_mut()
                    .find(|skill| skill.id == skill_id)
                    .unwrap();
                skill.attached = true;
                Ok(Value::Null)
            }
            ("POST", "/v1/search") => {
                let _email = self.email(&state)?;
                if let Some(error) = state.search_error.clone() {
                    return Err(error);
                }
                if let Some(results) = state.search_results.clone() {
                    return Ok(json!({ "searchEventId": "search_1", "results": results }));
                }
                let results: Vec<_> = state
                    .skills
                    .iter()
                    .filter(|skill| skill.attached)
                    .enumerate()
                    .map(|(index, skill)| {
                        json!({
                            "rank": index + 1,
                            "skillId": skill.id,
                            "score": 100 - index,
                            "frontmatter": {
                                "name": skill.name,
                                "description": skill.description
                            }
                        })
                    })
                    .collect();
                Ok(json!({ "searchEventId": "search_1", "results": results }))
            }
            ("POST", path) if path.starts_with("/v1/skills/") && path.ends_with("/load") => {
                let _email = self.email(&state)?;
                let skill_id = path
                    .trim_start_matches("/v1/skills/")
                    .trim_end_matches("/load");
                state.load_requests.push(skill_id.to_owned());
                if let Some(error) = state.load_error.clone() {
                    return Err(error);
                }
                let skill = state
                    .skills
                    .iter()
                    .find(|skill| skill.id == skill_id)
                    .unwrap();
                let paths = body
                    .as_ref()
                    .and_then(|body| body["paths"].as_array())
                    .filter(|paths| !paths.is_empty())
                    .cloned()
                    .unwrap_or_else(|| vec![Value::String("SKILL.md".into())]);
                let files: Vec<_> = paths
                    .iter()
                    .map(|path| {
                        let path = path.as_str().unwrap();
                        json!({ "path": path, "content": skill.files[path] })
                    })
                    .collect();
                Ok(json!({ "files": files }))
            }
            ("GET", path) if path.starts_with("/v1/collections/") && path.contains("/skills") => {
                let _email = self.email(&state)?;
                Ok(paged(skill_rows(&state.skills)))
            }
            ("GET", path) if path.starts_with("/v1/routers/") && path.contains("/skills") => {
                let _email = self.email(&state)?;
                let attached: Vec<_> = state
                    .skills
                    .iter()
                    .filter(|skill| skill.attached)
                    .cloned()
                    .collect();
                Ok(paged(skill_rows(&attached)))
            }
            _ => panic!("unhandled fake API request: {method} {path} {body:?}"),
        }
    }
}

fn publish_response(skill: &Skill) -> Value {
    json!({
        "skill": { "id": skill.id, "name": skill.name },
        "version": { "version": skill.version },
        "attachedRouterId": if skill.attached { Value::String("router_attached".into()) } else { Value::Null }
    })
}

fn skill_rows(skills: &[Skill]) -> Vec<Value> {
    skills
        .iter()
        .map(|skill| {
            json!({
                "skill": { "id": skill.id },
                "frontmatter": {
                    "name": skill.name,
                    "description": skill.description
                },
                "version": skill.version
            })
        })
        .collect()
}

struct TestRuntime {
    env: HashMap<String, String>,
    stdout: Vec<String>,
    stderr: Vec<String>,
    state: Arc<Mutex<Backend>>,
    approve_email: Option<String>,
    confirm_answer: bool,
    question: String,
}

impl Runtime for TestRuntime {
    fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    fn out(&mut self, text: &str) {
        self.stdout.push(text.to_owned());
    }

    fn err(&mut self, text: &str) {
        self.stderr.push(text.to_owned());
    }

    fn confirm(&mut self, question: &str) -> bool {
        self.question = question.to_owned();
        self.confirm_answer
    }

    fn open_browser(&mut self, _url: &str) {
        if let Some(email) = self.approve_email.clone() {
            self.state.lock().unwrap().approved_email = Some(email);
        }
    }

    fn sleep(&mut self, _duration: Duration) {}
}

struct Harness {
    _config: TempDir,
    env: HashMap<String, String>,
    state: Arc<Mutex<Backend>>,
    factory: FakeFactory,
}

struct ResultCapture {
    code: i32,
    stdout: String,
    stderr: String,
    question: String,
}

impl Harness {
    fn new() -> Self {
        let config = tempfile::tempdir().unwrap();
        let env = HashMap::from([
            ("FLOCKFLY_API_URL".into(), "http://fake-api.test".into()),
            (
                "FLOCKFLY_CONFIG_DIR".into(),
                config.path().display().to_string(),
            ),
        ]);
        let state = Arc::new(Mutex::new(Backend::default()));
        Self {
            _config: config,
            env,
            factory: FakeFactory(Arc::clone(&state)),
            state,
        }
    }

    fn run(
        &self,
        args: &[&str],
        approve_email: Option<&str>,
        confirm_answer: bool,
    ) -> ResultCapture {
        let mut runtime = TestRuntime {
            env: self.env.clone(),
            stdout: vec![],
            stderr: vec![],
            state: Arc::clone(&self.state),
            approve_email: approve_email.map(str::to_owned),
            confirm_answer,
            question: String::new(),
        };
        let code = run_cli_with(args, &mut runtime, &self.factory);
        ResultCapture {
            code,
            stdout: runtime.stdout.join("\n"),
            stderr: runtime.stderr.join("\n"),
            question: runtime.question,
        }
    }

    fn login(&self, email: &str) -> ResultCapture {
        self.run(&["login"], Some(email), false)
    }

    fn fixture(&self, name: &str) -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/skills")
            .join(name)
            .display()
            .to_string()
    }
}

#[test]
fn ts_logs_in_through_the_browser_flow_and_stores_a_token_without_printing_it() {
    let harness = Harness::new();
    let result = harness.login("jane@example.com");
    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("Open this URL in your browser"));
    assert!(result.stdout.contains("Logged in as jane@example.com"));
    assert!(!result.stdout.contains("ffly_"));

    let creds_path =
        std::path::Path::new(&harness.env["FLOCKFLY_CONFIG_DIR"]).join("credentials.json");
    let creds: Value = serde_json::from_str(&fs::read_to_string(&creds_path).unwrap()).unwrap();
    assert!(creds["token"].as_str().unwrap().starts_with("ffly_"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&creds_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    let whoami = harness.run(&["whoami"], None, false);
    assert_eq!(whoami.code, 0);
    assert!(whoami.stdout.contains("jane@example.com"));
    assert!(!whoami.stdout.contains(creds["token"].as_str().unwrap()));
}

#[test]
fn ts_prints_an_actionable_error_when_not_logged_in() {
    let result = Harness::new().run(&["whoami"], None, false);
    assert_eq!(result.code, 1);
    assert!(result.stderr.contains("flockfly login"));
}

#[test]
fn ts_prints_the_init_snippet_without_touching_files() {
    let result = Harness::new().run(&["init"], None, false);
    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("## Flockfly Skills"));
    assert!(result.stdout.contains("flockfly search \"<task>\""));
}

#[test]
fn ts_publishes_searches_and_loads_a_skill_end_to_end() {
    let harness = Harness::new();
    harness.login("pub@example.com");
    let publish = harness.run(&["publish", &harness.fixture("pdd")], None, false);
    assert_eq!(publish.code, 0);
    assert!(publish
        .stdout
        .contains("Published pdd as skill_1 (version 1)"));

    let routers = harness.run(&["routers", "list"], None, false);
    assert_eq!(routers.code, 0);
    assert!(routers.stdout.contains("pub's router"));
    let router_id = routers.stdout.split_whitespace().next().unwrap();
    let attach = harness.run(
        &["router", "add", "--skill", "skill_1", "--router", router_id],
        None,
        false,
    );
    assert_eq!(attach.code, 0);

    let search = harness.run(&["search", "transform an idea into a plan"], None, false);
    assert_eq!(search.code, 0);
    assert!(search.stdout.contains("1. skill_1"));
    assert!(search.stdout.contains("name: pdd"));
    assert!(!search.stdout.contains("# Prompt-Driven Development"));

    let load = harness.run(&["load", "skill_1"], None, false);
    assert_eq!(load.code, 0);
    assert!(load.stdout.contains("# Prompt-Driven Development"));
    assert!(!load.stdout.contains("# Detailed Design Template"));

    let multi = harness.run(
        &[
            "load",
            "skill_1",
            "references/design-template.md",
            "references/task-template.md",
        ],
        None,
        false,
    );
    assert_eq!(multi.code, 0);
    assert!(multi
        .stdout
        .contains("--- references/design-template.md ---"));
    assert!(multi.stdout.contains("--- references/task-template.md ---"));
}

#[test]
fn ts_publishes_with_router_and_finds_the_skill_via_search_immediately() {
    let harness = Harness::new();
    harness.login("router-pub@example.com");
    let publish = harness.run(
        &[
            "publish",
            &harness.fixture("codebase-summary"),
            "--router",
            "router-pub's router",
        ],
        None,
        false,
    );
    assert_eq!(publish.code, 0);
    assert!(publish.stdout.contains("Attached to router"));
    let search = harness.run(&["search", "analyze codebase documentation"], None, false);
    assert!(search.stdout.contains("codebase-summary"));
}

#[test]
fn ts_asks_before_replacing_an_existing_skill_and_honors_the_answer() {
    let harness = Harness::new();
    harness.login("replace@example.com");
    let fixture = harness.fixture("pdd");
    harness.run(&["publish", &fixture], None, false);

    let declined = harness.run(&["publish", &fixture], None, false);
    assert_eq!(declined.code, 1);
    assert!(declined.stderr.contains("Publish cancelled"));

    let accepted = harness.run(&["publish", &fixture], None, true);
    assert_eq!(accepted.code, 0);
    assert!(accepted.question.contains("already exists"));
    assert!(accepted.stdout.contains("(version 2)"));
}

#[test]
fn ts_prints_actionable_errors_for_unknown_routers_and_invalid_packages() {
    let harness = Harness::new();
    harness.login("errors@example.com");
    let bad_router = harness.run(
        &[
            "publish",
            &harness.fixture("pdd"),
            "--router",
            "nonexistent-router",
        ],
        None,
        false,
    );
    assert_eq!(bad_router.code, 1);
    assert!(bad_router.stderr.contains("flockfly routers list"));

    let bad_dir = harness.run(&["publish", "/nonexistent/skill-dir"], None, false);
    assert_eq!(bad_dir.code, 1);
    assert!(bad_dir.stderr.contains("Path not found"));
}

#[test]
fn ts_lists_public_collection_and_router_skills() {
    let harness = Harness::new();
    harness.login("lists@example.com");
    harness.run(
        &[
            "publish",
            &harness.fixture("pdd"),
            "--router",
            "lists's router",
        ],
        None,
        false,
    );
    let collection_list = harness.run(&["skills", "list"], None, false);
    assert_eq!(collection_list.code, 0);
    assert!(collection_list.stdout.contains("pdd v1"));
    let router_list = harness.run(
        &["skills", "list", "--router", "lists's router"],
        None,
        false,
    );
    assert_eq!(router_list.code, 0);
    assert!(router_list.stdout.contains("pdd v1"));
}

#[test]
fn search_help_documents_the_load_flag() {
    let result = Harness::new().run(&["search", "--help"], None, false);

    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("--load"));
}

#[test]
fn search_load_selects_the_best_rank_and_prints_raw_loaded_content() {
    let harness = Harness::new();
    harness.login("search-load@example.com");
    harness.run(
        &[
            "publish",
            &harness.fixture("pdd"),
            "--router",
            "search-load's router",
        ],
        None,
        false,
    );
    harness.run(
        &[
            "publish",
            &harness.fixture("codebase-summary"),
            "--router",
            "search-load's router",
        ],
        None,
        false,
    );
    harness.state.lock().unwrap().search_results = Some(vec![
        json!({
            "rank": 2,
            "skillId": "skill_1",
            "score": 80,
            "frontmatter": { "name": "pdd", "description": "Plan." }
        }),
        json!({
            "rank": 1,
            "skillId": "skill_2",
            "score": 100,
            "frontmatter": { "name": "codebase-summary", "description": "Analyze." }
        }),
    ]);

    let result = harness.run(&["search", "analyze code", "--load"], None, false);

    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("# Codebase Summary"));
    assert!(!result.stdout.contains("1. skill_2"));
    assert_eq!(harness.state.lock().unwrap().load_requests, vec!["skill_2"]);
    let standalone = harness.run(&["load", "skill_2"], None, false);
    assert_eq!(result.stdout, standalone.stdout);
}

#[test]
fn search_without_load_keeps_ranked_output_and_does_not_load() {
    let harness = Harness::new();
    harness.login("ordinary-search@example.com");
    harness.run(
        &[
            "publish",
            &harness.fixture("pdd"),
            "--router",
            "ordinary-search's router",
        ],
        None,
        false,
    );

    let result = harness.run(&["search", "plan work"], None, false);

    assert_eq!(result.code, 0);
    assert!(result.stdout.contains("1. skill_1"));
    assert!(result.stdout.contains("name: pdd"));
    assert!(harness.state.lock().unwrap().load_requests.is_empty());
}

#[test]
fn search_load_preserves_empty_results_without_a_load_request() {
    let harness = Harness::new();
    harness.login("empty-search@example.com");

    let result = harness.run(&["search", "nothing matches", "--load"], None, false);

    assert_eq!(result.code, 0);
    assert_eq!(result.stdout, "No matching skills found.");
    assert!(harness.state.lock().unwrap().load_requests.is_empty());
}

#[test]
fn search_load_propagates_search_api_failures_without_loading() {
    let harness = Harness::new();
    harness.login("search-failure@example.com");
    harness.state.lock().unwrap().search_error = Some(CliError::message("Search unavailable."));

    let result = harness.run(&["search", "anything", "--load"], None, false);

    assert_eq!(result.code, 1);
    assert!(result.stderr.contains("Error: Search unavailable."));
    assert!(harness.state.lock().unwrap().load_requests.is_empty());
}

#[test]
fn search_load_propagates_load_api_failures() {
    let harness = Harness::new();
    harness.login("load-failure@example.com");
    harness.run(
        &[
            "publish",
            &harness.fixture("pdd"),
            "--router",
            "load-failure's router",
        ],
        None,
        false,
    );
    harness.state.lock().unwrap().load_error = Some(CliError::message("Load unavailable."));

    let result = harness.run(&["search", "plan work", "--load"], None, false);

    assert_eq!(result.code, 1);
    assert!(result.stderr.contains("Error: Load unavailable."));
    assert_eq!(harness.state.lock().unwrap().load_requests, vec!["skill_1"]);
}
