use clap::Arg;
use clap::value_parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Url;
use std::path::PathBuf;
use std::str;
mod git;
use crate::git::find_new_repos;
use crate::git::list_github_team_repos;
use git::{LocalRepo, RemoteRepo, list_local_repos};
use log::error;
use tokio::task::JoinHandle;

fn parse_command_line_arguments() -> clap::ArgMatches {
    clap::Command::new("reposync")
        .about("tool to keep team repos up to date.")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::new("github_team_repo_url")
                .short('u')
                .long("github_team_repo_url")
                .env("GITHUB_TEAM_REPO_URL")
                .required(true)
                .value_parser(value_parser!(Url))
                .help("Points to github repo list. e.g. https://api.github.com/organizations/[organization_id]/team/[team_id]/repos."),
        )
        .arg(
            Arg::new("repo_root_dir")
                .short('d')
                .long("repo_root_dir")
                .env("REPO_ROOT_DIR")
                .required(true)
                .value_parser(value_parser!(PathBuf))
                .help("It has to point to the directory with all repos."),
        )
        .arg(
            Arg::new("github_token")
                .short('t')
                .long("github_token")
                .env("GITHUB_TOKEN")
                .required(true)
                .hide_env_values(true)
                .help("Github token with permissions to list all team repos."),
        )
        .arg(
            Arg::new("github_team_prefix")
                .short('p')
                .long("github_team_prefix")
                .env("GITHUB_TEAM_PREFIX")
                .required(true)
                .help("e.g. [team_] When cloning this prefix would be removed. If your team does not use it, set it to empty."),
        )
        .get_matches()
}

fn handle_new_pull(
    multi_progress: &MultiProgress,
    local_repo: LocalRepo,
) -> JoinHandle<Option<String>> {
    let spinner_style = ProgressStyle::with_template("{wide_msg}").unwrap();
    let bar = multi_progress.add(ProgressBar::new(10));
    bar.set_style(spinner_style.clone());
    bar.set_message(format!("{}: waiting...", local_repo.name));
    let handle = tokio::task::spawn_blocking(move || {
        bar.set_message(format!("{}: pulling...", local_repo.name));
        let _ = match git::git_pull(local_repo.clone()) {
            Err(message) => {
                bar.finish_with_message(format!("{}: updating failed", local_repo.name));
                return Some(format!("{}: {}", local_repo.name, message));
            }
            Ok(output) => {
                let error_message = str::from_utf8(output.stderr.trim_ascii()).unwrap();
                let info_message = str::from_utf8(output.stdout.trim_ascii()).unwrap();
                if !error_message.is_empty() {
                    bar.finish_with_message(format!("{}: updating failed", local_repo.name));
                    return Some(format!("{}: {}", local_repo.name, error_message));
                } else if info_message != "Already up to date."
                    && !info_message.contains("is up to date")
                {
                    bar.finish_with_message(format!("{}: updated", local_repo.name));
                }
                return None;
            }
        };
    });
    return handle;
}

fn handle_new_clone(
    multi_progress: &MultiProgress,
    repo_root_dir: &PathBuf,
    github_team_prefix: &String,
    new_repo: RemoteRepo,
) -> JoinHandle<Option<String>> {
    let repo_root_dir_clone = repo_root_dir.clone();
    let github_team_prefix_clone = github_team_prefix.clone();

    let spinner_style = ProgressStyle::with_template("{wide_msg}").unwrap();
    let bar = multi_progress.add(ProgressBar::new(10));
    bar.set_style(spinner_style.clone());
    bar.set_message(format!("{}: waiting...", new_repo.name));

    let handle = tokio::task::spawn_blocking(move || {
        bar.set_message(format!("{}: cloning...", new_repo.name));
        let _ = match git::git_clone(
            &new_repo.clone(),
            repo_root_dir_clone,
            github_team_prefix_clone,
        ) {
            Ok(_) => {
                bar.finish_with_message(format!("{}: cloned", new_repo.name));
                return None;
            }
            Err(message) => {
                bar.finish_with_message(format!("{}: cloing failed", new_repo.name));
                return Some(format!("{}: {}", new_repo.name, message));
            }
        };
    });
    return handle;
}

#[tokio::main]
async fn main() {
    let cli = parse_command_line_arguments();

    let repo_root_dir = cli.get_one::<PathBuf>("repo_root_dir").unwrap();
    let token = cli.get_one::<String>("github_token").unwrap();
    let github_team_repo_url = cli.get_one::<Url>("github_team_repo_url").unwrap();
    let github_team_prefix = cli.get_one::<String>("github_team_prefix").unwrap();

    let local_repos = list_local_repos(&repo_root_dir);
    let github_team_repos =
        list_github_team_repos(&token, &github_team_repo_url, &github_team_prefix).await;
    let new_repos = find_new_repos(&github_team_repos, &local_repos, &github_team_prefix);

    let multi_progress_bar = MultiProgress::new();
    simple_logger::init().unwrap();
    let mut threads: Vec<JoinHandle<Option<String>>> = Vec::new();

    for local_repo in local_repos.clone() {
        threads.push(handle_new_pull(&multi_progress_bar, local_repo));
    }


    for new_repo in new_repos.clone() {
        threads.push(handle_new_clone(
            &multi_progress_bar,
            repo_root_dir,
            github_team_prefix,
            new_repo,
        ));
    }

    let results = futures::future::join_all(threads).await;
    for result in results {
        if let Ok(Some(message)) = result {
            error!("{}", message);
        }
    }
}
