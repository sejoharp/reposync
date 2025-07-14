use clap::Arg;
use clap::value_parser;
use indicatif::MultiProgress;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use reqwest::Url;
use std::path::PathBuf;
use std::str;
mod git;
use git::{LocalRepo, RemoteRepo, list_local_repos};
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

#[derive(Debug)]
enum State {
    CloneError,
    PullError,
    Updated,
    Cloned,
    PullNoOp,
}

#[derive(Debug)]
struct GitResponse {
    name: String,
    message: String,
    state: State,
}
fn handle_new_pull(local_repo: LocalRepo) -> JoinHandle<GitResponse> {
    let handle = tokio::task::spawn_blocking(move || {
        let response = git::git_pull(local_repo.clone());
        let _ = match response {
            Err(message) => {
                return GitResponse {
                    name: local_repo.name,
                    message: message.to_string(),
                    state: State::PullError,
                };
            }
            Ok(output) => {
                let error_message = str::from_utf8(output.stderr.trim_ascii()).unwrap();
                let info_message = str::from_utf8(output.stdout.trim_ascii()).unwrap();
                if !error_message.is_empty() {
                    return GitResponse {
                        name: local_repo.name,
                        message: error_message.to_string(),
                        state: State::PullError,
                    };
                } else if info_message != "Already up to date."
                    && !info_message.contains("is up to date")
                    && !info_message.contains("[new tag]")
                {
                    return GitResponse {
                        name: local_repo.name,
                        message: info_message.to_string(),
                        state: State::Updated,
                    };
                }
                return GitResponse {
                    name: local_repo.name,
                    message: "".into(),
                    state: State::PullNoOp,
                };
            }
        };
    });
    return handle;
}

fn handle_new_clone(
    repo_root_dir: &PathBuf,
    github_team_prefix: &String,
    new_repo: RemoteRepo,
) -> JoinHandle<GitResponse> {
    let repo_root_dir_clone = repo_root_dir.clone();
    let github_team_prefix_clone = github_team_prefix.clone();

    let handle = tokio::task::spawn_blocking(move || {
        let _ = match git::git_clone(
            &new_repo.clone(),
            repo_root_dir_clone,
            github_team_prefix_clone,
        ) {
            Ok(_) => {
                return GitResponse {
                    name: new_repo.name,
                    message: "".into(),
                    state: State::Cloned,
                };
            }
            Err(message) => {
                return GitResponse {
                    name: new_repo.name,
                    message: message.to_string(),
                    state: State::CloneError,
                };
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

    let multi_progress_bar = MultiProgress::new();
    let spinner_style = ProgressStyle::with_template("{wide_msg}").unwrap();
    let pull_progress_bar = multi_progress_bar.add(ProgressBar::no_length());
    pull_progress_bar.set_style(spinner_style.clone());
    let clone_progress_bar = multi_progress_bar.add(ProgressBar::no_length());
    clone_progress_bar.set_style(spinner_style.clone());

    let mut clone_threads: Vec<JoinHandle<GitResponse>> = Vec::new();
    let mut pull_threads: Vec<JoinHandle<GitResponse>> = Vec::new();

    pull_progress_bar.set_message(format!("gathering local repos..."));
    let local_repos = list_local_repos(&repo_root_dir);
    pull_progress_bar.set_message(format!("pulling repos..."));
    for local_repo in local_repos.clone() {
        pull_threads.push(handle_new_pull(local_repo));
    }

    clone_progress_bar.set_message("looking for new team repos...");
    let remote_repos = git::get_all_repos(token, github_team_prefix, github_team_repo_url).await;
    let github_active_team_repos = git::list_active_github_team_repos(remote_repos.clone()).await;
    let new_repos =
        git::find_new_repos(&github_active_team_repos, &local_repos, &github_team_prefix);
    clone_progress_bar.set_message("cloning team repos...");
    for new_repo in new_repos.clone() {
        clone_threads.push(handle_new_clone(
            repo_root_dir,
            github_team_prefix,
            new_repo,
        ));
    }

    let github_archived_team_repos =
        git::list_archived_github_team_repos(remote_repos.clone()).await;
    let archived_repos = git::find_archived_local_repos(
        &github_archived_team_repos,
        &local_repos,
        &github_team_prefix,
    );

    let mut pull_errors: Vec<GitResponse> = Vec::new();
    let mut pull_noop: Vec<GitResponse> = Vec::new();
    let mut updated: Vec<GitResponse> = Vec::new();
    let mut cloned: Vec<GitResponse> = Vec::new();
    let mut clone_errors: Vec<GitResponse> = Vec::new();
    for pull_thread in pull_threads {
        let pull_result = pull_thread.await.unwrap();
        match pull_result.state {
            State::PullError => {
                pull_errors.push(pull_result);
            }
            State::PullNoOp => {
                pull_noop.push(pull_result);
            }
            State::Updated => {
                updated.push(pull_result);
            }
             _ => {
                panic!("Unexpected state in pull thread: {:?}", pull_result);
            }
        };
    }
    pull_progress_bar.set_message("pulling finished");
    pull_progress_bar.finish_and_clear();

    for clone_thread in clone_threads {
        let clone_result = clone_thread.await.unwrap();
        match clone_result.state {
            State::CloneError => {
                clone_errors.push(clone_result);
            }
            State::Cloned => {
                cloned.push(clone_result);
            }
            _ => {
                panic!("Unexpected state in clone thread: {:?}", clone_result);
            }
        };
    }
    clone_progress_bar.set_message("cloning finished");
    clone_progress_bar.finish_and_clear();

    println!(
        "\x1b[32mPull no-op count\x1b[0m: {}",
        pull_noop.iter().count()
    );
    for updated_repo in updated {
        println!("\x1b[33m{}\x1b[0m: updated", updated_repo.name);
    }
    for cloned_repo in cloned {
        println!("\x1b[33m{}\x1b[0m: cloned", cloned_repo.name);
    }
    for archived_repo in archived_repos {
        println!("\x1b[33m{}\x1b[0m: archived", archived_repo.name);
    }
    for clone_error in clone_errors {
        println!("\x1b[31m{}\x1b[0m: failed to clone:", clone_error.name);
        for line in clone_error.message.lines() {
            println!("  {}", line);
        }
    }
    for pull_error in pull_errors {
        println!("\x1b[31m{}\x1b[0m: failed to pull:", pull_error.name);
        for line in pull_error.message.lines() {
            println!("  {}", line);
        }
    }
}
