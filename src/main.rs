use clap::Arg;
use clap::value_parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Url;
use std::path::PathBuf;
use std::str;
mod git;
use tokio::task::JoinHandle;

use crate::git::find_new_repos;
use crate::git::list_github_team_repos;



struct GitMessage {
    name: String,
    message: String,
}
use git::{LocalRepo, RemoteRepo, list_local_repos};

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

fn handle_pull(
    pull_pb: &ProgressBar,
    new_commits_pb: &ProgressBar,
    local_repo: LocalRepo,
) -> JoinHandle<Result<GitMessage, GitMessage>> {
    let pull_pb_clone = pull_pb.clone();
    let new_commits_pb_clone = new_commits_pb.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let _ = match git::git_pull(local_repo.clone()) {
            Err(message) => {
                return Err(GitMessage {
                    name: local_repo.name,
                    message: format!("Pulling failed with message:{}", message),
                });
            }
            Ok(output) => {
                if str::from_utf8(output.stderr.trim_ascii()).unwrap() != "" {
                    return Err(GitMessage {
                        name: local_repo.name,
                        message: str::from_utf8(output.stderr.trim_ascii())
                            .unwrap()
                            .to_string(),
                    });
                } else {
                    pull_pb_clone.inc(1);
                    if str::from_utf8(output.stdout.trim_ascii()).unwrap() != "Already up to date."
                    {
                        new_commits_pb_clone.inc(1);
                        return Ok(GitMessage {
                            name: local_repo.name,
                            message: "updated".to_string(),
                        });
                    } else {
                        return Ok(GitMessage {
                            name: local_repo.name,
                            message: "".to_string(),
                        });
                    }
                }
            }
        };
    });
    return handle;
}

fn handle_clone(
    clone_pb: &ProgressBar,
    repo_root_dir: &PathBuf,
    github_team_prefix: &String,
    new_repo: RemoteRepo,
) -> JoinHandle<Result<GitMessage, GitMessage>> {
    let clone_pb_clone = clone_pb.clone();
    let repo_root_dir_clone = repo_root_dir.clone();
    let github_team_prefix_clone = github_team_prefix.clone();
    let handle = tokio::task::spawn_blocking(move || {
        let _ = git::git_clone(
            &new_repo.clone(),
            repo_root_dir_clone,
            github_team_prefix_clone,
        );
        clone_pb_clone.inc(1);
        return Ok(GitMessage {
            name: new_repo.clone().name,
            message: "cloned".to_string(),
        });
    });
    return handle;
}

fn create_progressbar(
    multi_progress_bar: &MultiProgress,
    size: usize,
    bar_prefix: String,
) -> ProgressBar {
    let style_clone = ProgressStyle::with_template(&format!(
        "{}: {{bar:40.cyan/blue}} {{pos:>7}}/{{len:7}} {{msg}}",
        bar_prefix
    ))
    .unwrap()
    .progress_chars("##-");
    let clone_pb = multi_progress_bar.add(ProgressBar::new(size as u64));
    clone_pb.set_style(style_clone);
    return clone_pb;
}

fn print_git_message_with_separator(messages: Vec<GitMessage>, title_prefix: String) {
    for message in messages {
        if message.message.is_empty() {
            continue;
        }
        println!(
            "===================================== {} in {} ====================================",
            title_prefix, message.name
        );
        println!("{}", message.message);
    }
}
fn print_git_message(messages: Vec<GitMessage>) {
    for message in messages {
        if message.message.is_empty() {
            continue;
        }
        println!("{} {}", message.name, message.message);
    }
}

#[tokio::main]
async fn main() {
    let cli = parse_command_line_arguments();

    let repo_root_dir = cli.get_one::<PathBuf>("repo_root_dir").unwrap();
    let token = cli.get_one::<String>("github_token").unwrap();
    let github_team_repo_url = cli.get_one::<Url>("github_team_repo_url").unwrap();
    let github_team_prefix = cli.get_one::<String>("github_team_prefix").unwrap();

    let local_repos = list_local_repos(&repo_root_dir);

    let multi_progress_bar = MultiProgress::new();

    let mut threads: Vec<JoinHandle<Result<GitMessage, GitMessage>>> = Vec::new();

    let pull_pb = create_progressbar(
        &multi_progress_bar,
        local_repos.len(),
        "pulling ".to_string(),
    );
    let new_commits_pb = create_progressbar(&multi_progress_bar, 0, "Updating".to_string());
    for local_repo in local_repos.clone() {
        threads.push(handle_pull(&pull_pb, &new_commits_pb, local_repo));
    }

    let github_team_repos =
        list_github_team_repos(&token, &github_team_repo_url, &github_team_prefix).await;
    let new_repos = find_new_repos(&github_team_repos, &local_repos, &github_team_prefix);

    let clone_pb = create_progressbar(&multi_progress_bar, new_repos.len(), "cloning ".to_string());
    for new_repo in new_repos.clone() {
        threads.push(handle_clone(
            &clone_pb,
            repo_root_dir,
            github_team_prefix,
            new_repo,
        ));
    }

    let mut error_messages: Vec<GitMessage> = Vec::new();
    let mut ok_messages: Vec<GitMessage> = Vec::new();
    for thread in threads {
        let _ = match thread.await.unwrap() {
            Err(error_message) => {
                error_messages.push(error_message);
            }
            Ok(message) => {
                ok_messages.push(message);
            }
        };
    }

    pull_pb.finish_with_message("done");
    clone_pb.finish_with_message("done");
    new_commits_pb.finish_with_message("done");

    print_git_message(ok_messages);
    print_git_message_with_separator(error_messages, "ERROR in".to_string());
}
