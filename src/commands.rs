use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use serde_json::{json, Value};

use crate::api::{Api, ApiFactory, CliError};
use crate::config::{api_url, load_credentials, save_credentials, Credentials};
use crate::format::{
    format_loaded_files, format_search_results, LoadedFile, SearchResult, INIT_SNIPPET,
};
use crate::package::package_skill_directory;

pub trait Runtime {
    fn env(&self) -> &HashMap<String, String>;
    fn out(&mut self, text: &str);
    fn err(&mut self, text: &str);
    fn confirm(&mut self, question: &str) -> bool;
    fn open_browser(&mut self, url: &str);
    fn sleep(&mut self, duration: Duration);
}

#[derive(Parser)]
#[command(
    name = "flockfly",
    version,
    about = "Flockfly context router CLI: publish, search, and load skills."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate with the Flockfly service through your browser
    Login,
    /// Show the authenticated user
    Whoami,
    /// Print the Flockfly discovery instructions
    Init,
    /// Publish a skill package directory to the public collection
    Publish {
        /// Directory containing SKILL.md plus referenced files
        skill_directory: String,
        /// Also attach the published skill to this router
        #[arg(long)]
        router: Option<String>,
        /// Replace an existing skill with the same name without prompting
        #[arg(long)]
        yes: bool,
    },
    /// Search skills routed to you
    Search {
        /// Task description to search for
        query: String,
        /// Immediately load and print the highest-ranked skill
        #[arg(long)]
        load: bool,
    },
    /// Load SKILL.md (default) or specific files from a skill package
    Load {
        /// Skill ID from `flockfly search`
        skill_id: String,
        /// Package paths to load instead of SKILL.md
        paths: Vec<String>,
    },
    /// Manage router skills
    Router {
        #[command(subcommand)]
        command: RouterCommand,
    },
    /// Work with your routers
    Routers {
        #[command(subcommand)]
        command: RoutersCommand,
    },
    /// Work with skills
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
}

#[derive(Subcommand)]
enum RouterCommand {
    /// Attach an existing skill to a router
    Add {
        /// Skill ID
        #[arg(long)]
        skill: String,
        /// Router ID or name
        #[arg(long)]
        router: String,
    },
}

#[derive(Subcommand)]
enum RoutersCommand {
    /// List routers you belong to
    List,
}

#[derive(Subcommand)]
enum SkillsCommand {
    /// List public-collection skills, or a router's skills with --router
    List {
        /// List skills attached to a router
        #[arg(long)]
        router: Option<String>,
    },
}

pub fn run_cli_with<S: AsRef<str>>(
    args: &[S],
    runtime: &mut dyn Runtime,
    factory: &dyn ApiFactory,
) -> i32 {
    let argv = std::iter::once("flockfly".to_owned())
        .chain(args.iter().map(|arg| arg.as_ref().to_owned()))
        .collect::<Vec<_>>();
    let cli = match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(error) => {
            let code = error.exit_code();
            let rendered = error.to_string();
            let rendered = rendered.trim_end_matches('\n');
            if error.use_stderr() {
                runtime.err(rendered);
            } else {
                runtime.out(rendered);
            }
            return code;
        }
    };

    match execute(cli.command, runtime, factory) {
        Ok(()) => 0,
        Err(error) => {
            runtime.err(&format!("Error: {error}"));
            1
        }
    }
}

fn execute(
    command: Command,
    runtime: &mut dyn Runtime,
    factory: &dyn ApiFactory,
) -> Result<(), CliError> {
    match command {
        Command::Login => login(runtime, factory),
        Command::Whoami => {
            let client = authed_client(runtime.env(), factory)?;
            let me = client.request("GET", "/v1/me", None)?;
            runtime.out(&format!(
                "{} (username: {}, org: {})",
                string_at(&me, &["user", "email"])?,
                string_at(&me, &["user", "username"])?,
                string_at(&me, &["org", "name"])?
            ));
            Ok(())
        }
        Command::Init => {
            runtime.out(INIT_SNIPPET);
            Ok(())
        }
        Command::Publish {
            skill_directory,
            router,
            yes,
        } => publish(runtime, factory, &skill_directory, router.as_deref(), yes),
        Command::Search { query, load } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = client.request("POST", "/v1/search", Some(json!({ "query": query })))?;
            let results = response
                .get("results")
                .and_then(Value::as_array)
                .ok_or_else(|| CliError::message("Invalid search response."))?
                .iter()
                .map(parse_search_result)
                .collect::<Result<Vec<_>, _>>()?;
            if load {
                if let Some(result) = results.iter().min_by_key(|result| result.rank) {
                    runtime.out(&load_skill(client.as_ref(), &result.skill_id, &[])?);
                } else {
                    runtime.out(&format_search_results(&results));
                }
            } else {
                runtime.out(&format_search_results(&results));
            }
            Ok(())
        }
        Command::Load { skill_id, paths } => {
            let client = authed_client(runtime.env(), factory)?;
            runtime.out(&load_skill(client.as_ref(), &skill_id, &paths)?);
            Ok(())
        }
        Command::Router {
            command: RouterCommand::Add { skill, router },
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let router_id = resolve_router_id(client.as_ref(), &router)?;
            client.request(
                "POST",
                &format!(
                    "/v1/routers/{}/skills/{}",
                    urlencoding::encode(&router_id),
                    urlencoding::encode(&skill)
                ),
                None,
            )?;
            runtime.out(&format!("Attached {skill} to router {router}."));
            Ok(())
        }
        Command::Routers {
            command: RoutersCommand::List,
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = client.request("GET", "/v1/routers", None)?;
            let routers = response
                .get("routers")
                .and_then(Value::as_array)
                .ok_or_else(|| CliError::message("Invalid routers response."))?;
            if routers.is_empty() {
                runtime.out("You have no routers yet.");
                return Ok(());
            }
            let rows = routers
                .iter()
                .map(|router| {
                    let id = string_at(router, &["id"])?;
                    let name = string_at(router, &["name"])?;
                    Ok(format!("{id}  {name}"))
                })
                .collect::<Result<Vec<_>, CliError>>()?;
            runtime.out(&rows.join("\n"));
            Ok(())
        }
        Command::Skills {
            command: SkillsCommand::List { router },
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let rows = if let Some(router) = router {
                let router_id = resolve_router_id(client.as_ref(), &router)?;
                fetch_all_skill_pages(
                    client.as_ref(),
                    &format!("/v1/routers/{}/skills", urlencoding::encode(&router_id)),
                )?
            } else {
                let collection = require_public_collection(client.as_ref())?;
                let collection_id = string_at(&collection, &["id"])?;
                fetch_all_skill_pages(
                    client.as_ref(),
                    &format!(
                        "/v1/collections/{}/skills",
                        urlencoding::encode(&collection_id)
                    ),
                )?
            };
            runtime.out(&render_skill_rows(&rows)?);
            Ok(())
        }
    }
}

fn login(runtime: &mut dyn Runtime, factory: &dyn ApiFactory) -> Result<(), CliError> {
    let env = runtime.env().clone();
    let base_url = api_url(&env).trim_end_matches('/').to_owned();
    let client = factory.create(&base_url, None);
    let start = client.request("POST", "/v1/auth/cli/start", None)?;
    let cli_auth_id = string_at(&start, &["cliAuthId"])?;
    let verification_url = string_at(&start, &["verificationUrl"])?;
    runtime.out(&format!(
        "Open this URL in your browser to log in:\n\n  {verification_url}\n"
    ));
    runtime.open_browser(&verification_url);

    let started = Instant::now();
    for _ in 0..200 {
        if started.elapsed() >= Duration::from_secs(5 * 60) {
            break;
        }
        let poll = client.request(
            "POST",
            "/v1/auth/cli/poll",
            Some(json!({ "cliAuthId": cli_auth_id })),
        )?;
        if poll.get("status").and_then(Value::as_str) == Some("approved") {
            if let Some(token) = poll.get("token").and_then(Value::as_str) {
                save_credentials(
                    &Credentials {
                        api_url: base_url.clone(),
                        token: token.to_owned(),
                    },
                    &env,
                )
                .map_err(|error| CliError::message(error.to_string()))?;
                let me = factory
                    .create(&base_url, Some(token))
                    .request("GET", "/v1/me", None)?;
                runtime.out(&format!(
                    "Logged in as {} (org: {}).",
                    string_at(&me, &["user", "email"])?,
                    string_at(&me, &["org", "name"])?
                ));
                return Ok(());
            }
        }
        runtime.sleep(Duration::from_millis(1500));
    }

    Err(CliError::message(
        "Login timed out. Run `flockfly login` to try again.",
    ))
}

fn publish(
    runtime: &mut dyn Runtime,
    factory: &dyn ApiFactory,
    directory: &str,
    router: Option<&str>,
    yes: bool,
) -> Result<(), CliError> {
    let client = authed_client(runtime.env(), factory)?;
    let packaged = package_skill_directory(Path::new(directory))?;
    let collection = require_public_collection(client.as_ref())?;
    let collection_id = string_at(&collection, &["id"])?;
    let router_id = router
        .map(|router| resolve_router_id(client.as_ref(), router))
        .transpose()?;

    let publish_once = |replace_skill_id: Option<&str>| {
        let mut body = json!({ "files": packaged.files });
        if let Some(router_id) = &router_id {
            body["routerId"] = Value::String(router_id.clone());
        }
        if let Some(replace_skill_id) = replace_skill_id {
            body["replaceSkillId"] = Value::String(replace_skill_id.to_owned());
            body["confirmReplace"] = Value::Bool(true);
        }
        client.request(
            "POST",
            &format!("/v1/collections/{collection_id}/skills"),
            Some(body),
        )
    };

    let result = match publish_once(None) {
        Ok(result) => result,
        Err(error) if error.code.as_deref() == Some("skill_name_conflict") => {
            let existing_skill_id = error
                .extra
                .as_ref()
                .and_then(|extra| extra.get("existingSkillId"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let next_version = error
                .extra
                .as_ref()
                .and_then(|extra| extra.get("nextVersion"))
                .map(ToString::to_string)
                .unwrap_or_else(|| "next".to_owned());
            let Some(existing_skill_id) = existing_skill_id else {
                return Err(error);
            };
            let replace = yes
                || runtime.confirm(&format!(
                    "A skill named {} already exists. Replace it as version {next_version}? [y/N] ",
                    packaged.frontmatter.name
                ));
            if !replace {
                return Err(CliError::message("Publish cancelled."));
            }
            publish_once(Some(&existing_skill_id))?
        }
        Err(error) => return Err(error),
    };

    runtime.out(&format!(
        "Published {} as {} (version {}).",
        string_at(&result, &["skill", "name"])?,
        string_at(&result, &["skill", "id"])?,
        value_at(&result, &["version", "version"])?
    ));
    if result
        .get("attachedRouterId")
        .is_some_and(|value| !value.is_null())
    {
        runtime.out(&format!(
            "Attached to router {}.",
            router.unwrap_or("undefined")
        ));
    }
    Ok(())
}

fn authed_client(
    env: &HashMap<String, String>,
    factory: &dyn ApiFactory,
) -> Result<Box<dyn Api>, CliError> {
    let credentials = load_credentials(env)
        .ok_or_else(|| CliError::message("You are not logged in. Run `flockfly login` first."))?;
    Ok(factory.create(&credentials.api_url, Some(&credentials.token)))
}

// Accepts a router ID or router name and resolves it against the user's routers.
fn resolve_router_id(client: &dyn Api, id_or_name: &str) -> Result<String, CliError> {
    let response = client.request("GET", "/v1/routers", None)?;
    let routers = response
        .get("routers")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::message("Invalid routers response."))?;
    routers
        .iter()
        .find(|router| {
            router.get("id").and_then(Value::as_str) == Some(id_or_name)
                || router.get("name").and_then(Value::as_str) == Some(id_or_name)
        })
        .and_then(|router| router.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CliError::message(format!(
                "Router not found: {id_or_name}. Run `flockfly routers list` to see your routers."
            ))
        })
}

fn require_public_collection(client: &dyn Api) -> Result<Value, CliError> {
    let response = client.request("GET", "/v1/collections", None)?;
    let collections = response
        .get("collections")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::message("Invalid collections response."))?;
    collections
        .iter()
        .find(|collection| collection.get("kind").and_then(Value::as_str) == Some("public"))
        .cloned()
        .ok_or_else(|| CliError::message("No public collection is available to publish into."))
}

// Loops a cursor-paginated GET endpoint (100 rows/page) until exhausted.
fn fetch_all_skill_pages(client: &dyn Api, path: &str) -> Result<Vec<Value>, CliError> {
    let mut rows = Vec::new();
    let mut seen_cursors = HashSet::new();
    let mut cursor: Option<String> = None;
    loop {
        let separator = if path.contains('?') { "&" } else { "?" };
        let page_path = match &cursor {
            Some(cursor) => format!(
                "{path}{separator}limit=100&cursor={}",
                urlencoding::encode(cursor)
            ),
            None => format!("{path}{separator}limit=100"),
        };
        let page = client.request("GET", &page_path, None)?;
        let skills = page
            .get("skills")
            .and_then(Value::as_array)
            .ok_or_else(|| CliError::message("Invalid skills response."))?;
        rows.extend(skills.iter().cloned());
        let next_cursor = page
            .get("page")
            .and_then(|page| page.get("nextCursor"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        match next_cursor {
            Some(next) => {
                if !seen_cursors.insert(next.clone()) {
                    return Err(CliError::message(
                        "The API returned a repeated page cursor.",
                    ));
                }
                cursor = Some(next);
            }
            None => break,
        }
    }
    Ok(rows)
}

fn parse_search_result(value: &Value) -> Result<SearchResult, CliError> {
    Ok(SearchResult {
        rank: value_at(value, &["rank"])?
            .as_u64()
            .ok_or_else(|| CliError::message("Invalid search rank."))?,
        skill_id: string_at(value, &["skillId"])?,
        score: value_at(value, &["score"])?.as_f64().unwrap_or(0.0),
        name: string_at(value, &["frontmatter", "name"])?,
        description: string_at(value, &["frontmatter", "description"])?,
    })
}

fn load_skill(client: &dyn Api, skill_id: &str, paths: &[String]) -> Result<String, CliError> {
    let response = client.request(
        "POST",
        &format!("/v1/skills/{}/load", urlencoding::encode(skill_id)),
        Some(json!({ "paths": paths })),
    )?;
    let files: Vec<LoadedFile> = serde_json::from_value(
        response
            .get("files")
            .cloned()
            .ok_or_else(|| CliError::message("Invalid load response."))?,
    )
    .map_err(|error| CliError::message(error.to_string()))?;
    Ok(format_loaded_files(&files))
}

fn render_skill_rows(rows: &[Value]) -> Result<String, CliError> {
    if rows.is_empty() {
        return Ok("No skills found.".to_owned());
    }
    rows.iter()
        .map(|row| {
            Ok(format!(
                "{}  {} v{}\n    {}",
                string_at(row, &["skill", "id"])?,
                string_at(row, &["frontmatter", "name"])?,
                value_at(row, &["version"])?,
                string_at(row, &["frontmatter", "description"])?
            ))
        })
        .collect::<Result<Vec<_>, CliError>>()
        .map(|rows| rows.join("\n"))
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Result<&'a Value, CliError> {
    path.iter().try_fold(value, |current, segment| {
        current
            .get(segment)
            .ok_or_else(|| CliError::message(format!("Invalid API response: missing {segment}.")))
    })
}

fn string_at(value: &Value, path: &[&str]) -> Result<String, CliError> {
    value_at(value, path)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| CliError::message("Invalid API response: expected string."))
}
