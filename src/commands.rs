use std::collections::HashMap;
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
    about = "Flockfly context router CLI: publish, search, and load team skills."
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
    /// Print the snippet to add to CLAUDE.md or AGENTS.md
    Init,
    /// Publish a skill package directory
    Publish {
        /// Directory containing SKILL.md plus referenced files
        skill_directory: String,
        /// Also attach the published skill to this team
        #[arg(long)]
        team: Option<String>,
        /// Replace an existing skill with the same name without prompting
        #[arg(long)]
        yes: bool,
    },
    /// Search your visible skills
    Search {
        /// Task description to search for
        query: String,
    },
    /// Load SKILL.md (default) or specific files from a skill package
    Load {
        /// Skill ID from `flockfly search`
        skill_id: String,
        /// Package paths to load instead of SKILL.md
        paths: Vec<String>,
    },
    /// Manage team skills
    Team {
        #[command(subcommand)]
        command: TeamCommand,
    },
    /// Work with your teams
    Teams {
        #[command(subcommand)]
        command: TeamsCommand,
    },
    /// Work with skills
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
}

#[derive(Subcommand)]
enum TeamCommand {
    /// Attach an existing org skill to a team
    Add {
        /// Skill ID
        #[arg(long)]
        skill: String,
        /// Team ID or name
        #[arg(long)]
        team: String,
    },
}

#[derive(Subcommand)]
enum TeamsCommand {
    /// List teams you belong to
    List,
}

#[derive(Subcommand)]
enum SkillsCommand {
    /// List org skills, or a team's skills with --team
    List {
        /// List all org skills (default)
        #[arg(long)]
        org: bool,
        /// List skills attached to a team
        #[arg(long)]
        team: Option<String>,
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
            runtime.out(&format!(
                "Add the following snippet to your CLAUDE.md or AGENTS.md:\n\n{INIT_SNIPPET}"
            ));
            Ok(())
        }
        Command::Publish {
            skill_directory,
            team,
            yes,
        } => publish(runtime, factory, &skill_directory, team.as_deref(), yes),
        Command::Search { query } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = client.request("POST", "/v1/search", Some(json!({ "query": query })))?;
            let results = response
                .get("results")
                .and_then(Value::as_array)
                .ok_or_else(|| CliError::message("Invalid search response."))?
                .iter()
                .map(parse_search_result)
                .collect::<Result<Vec<_>, _>>()?;
            runtime.out(&format_search_results(&results));
            Ok(())
        }
        Command::Load { skill_id, paths } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = client.request(
                "POST",
                &format!("/v1/skills/{}/load", urlencoding::encode(&skill_id)),
                Some(json!({ "paths": paths })),
            )?;
            let files: Vec<LoadedFile> = serde_json::from_value(
                response
                    .get("files")
                    .cloned()
                    .ok_or_else(|| CliError::message("Invalid load response."))?,
            )
            .map_err(|error| CliError::message(error.to_string()))?;
            runtime.out(&format_loaded_files(&files));
            Ok(())
        }
        Command::Team {
            command: TeamCommand::Add { skill, team },
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let team_id = resolve_team_id(client.as_ref(), &team)?;
            client.request(
                "POST",
                &format!(
                    "/v1/teams/{}/skills/{}",
                    urlencoding::encode(&team_id),
                    urlencoding::encode(&skill)
                ),
                None,
            )?;
            runtime.out(&format!("Attached {skill} to team {team}."));
            Ok(())
        }
        Command::Teams {
            command: TeamsCommand::List,
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = client.request("GET", "/v1/teams", None)?;
            let teams = response
                .get("teams")
                .and_then(Value::as_array)
                .ok_or_else(|| CliError::message("Invalid teams response."))?;
            if teams.is_empty() {
                runtime.out("You have no teams yet.");
                return Ok(());
            }
            let rows = teams
                .iter()
                .map(|team| {
                    let id = string_at(team, &["id"])?;
                    let name = string_at(team, &["name"])?;
                    let personal = if team.get("kind").and_then(Value::as_str) == Some("personal") {
                        "[personal] "
                    } else {
                        ""
                    };
                    Ok(format!("{id}  {personal}{name}"))
                })
                .collect::<Result<Vec<_>, CliError>>()?;
            runtime.out(&rows.join("\n"));
            Ok(())
        }
        Command::Skills {
            command: SkillsCommand::List { org: _, team },
        } => {
            let client = authed_client(runtime.env(), factory)?;
            let response = if let Some(team) = team {
                let team_id = resolve_team_id(client.as_ref(), &team)?;
                client.request(
                    "GET",
                    &format!("/v1/teams/{}/skills", urlencoding::encode(&team_id)),
                    None,
                )?
            } else {
                let me = client.request("GET", "/v1/me", None)?;
                let org_id = string_at(&me, &["org", "id"])?;
                client.request("GET", &format!("/v1/orgs/{org_id}/skills"), None)?
            };
            let skills = response
                .get("skills")
                .and_then(Value::as_array)
                .ok_or_else(|| CliError::message("Invalid skills response."))?;
            runtime.out(&render_skill_rows(skills)?);
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
    team: Option<&str>,
    yes: bool,
) -> Result<(), CliError> {
    let client = authed_client(runtime.env(), factory)?;
    let me = client.request("GET", "/v1/me", None)?;
    let org_id = string_at(&me, &["org", "id"])?;
    let packaged = package_skill_directory(Path::new(directory))?;
    let team_id = team
        .map(|team| resolve_team_id(client.as_ref(), team))
        .transpose()?;

    let publish_once = |replace_skill_id: Option<&str>| {
        let mut body = json!({ "files": packaged.files });
        if let Some(team_id) = &team_id {
            body["teamId"] = Value::String(team_id.clone());
        }
        if let Some(replace_skill_id) = replace_skill_id {
            body["replaceSkillId"] = Value::String(replace_skill_id.to_owned());
            body["confirmReplace"] = Value::Bool(true);
        }
        client.request("POST", &format!("/v1/orgs/{org_id}/skills"), Some(body))
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
        .get("attachedTeamId")
        .is_some_and(|value| !value.is_null())
    {
        runtime.out(&format!(
            "Attached to team {}.",
            team.unwrap_or("undefined")
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

fn resolve_team_id(client: &dyn Api, id_or_name: &str) -> Result<String, CliError> {
    let response = client.request("GET", "/v1/teams", None)?;
    let teams = response
        .get("teams")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::message("Invalid teams response."))?;
    teams
        .iter()
        .find(|team| {
            team.get("id").and_then(Value::as_str) == Some(id_or_name)
                || team.get("name").and_then(Value::as_str) == Some(id_or_name)
        })
        .and_then(|team| team.get("id"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            CliError::message(format!(
                "Team not found: {id_or_name}. Run `flockfly teams list` to see your teams."
            ))
        })
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
