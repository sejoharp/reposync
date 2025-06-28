use clap::Arg;
use clap::value_parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use reqwest::Url;
use reqwest::header::ACCEPT;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::str;
use std::{ffi::OsStr, fs, path::PathBuf, process::Command};
use tokio::task::JoinHandle;

fn is_git_repo(path: &String) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            match entry {
                Ok(entry) => {
                    if entry
                        .path()
                        .file_name()
                        .is_some_and(|subdir| subdir.eq(OsStr::new(".git")))
                    {
                        return true;
                    }
                }
                Err(_e) => (),
            }
        }
    }
    false
}

fn git_pull(local_repo: LocalRepo) -> Result<std::process::Output, std::io::Error> {
    return Command::new("git")
        .arg("pull")
        .current_dir(local_repo.path)
        .output();
}

fn git_clone(
    remote_repo: &RemoteRepo,
    repo_root_dir: PathBuf,
    github_team_prefix: String,
) -> Result<std::process::Output, std::io::Error> {
    return Command::new("git")
        .arg("clone")
        .arg(remote_repo.git_url.clone())
        .arg(remote_repo.name.replace(github_team_prefix.as_str(), ""))
        .current_dir(repo_root_dir)
        .output();
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RemoteRepo {
    name: String,
    archived: bool,
    git_url: String,
}

async fn get_repos(
    client: &Client,
    token: &String,
    page: i32,
    github_team_repo_url: &Url,
) -> Option<Vec<RemoteRepo>> {
    let repos = client
        .get(github_team_repo_url.clone())
        .header(ACCEPT, "application/vnd.github.v3+json")
        .header(USER_AGENT, "reposync")
        .bearer_auth(token)
        .query(&[("per_page", "100"), ("page", page.to_string().as_str())])
        .send()
        .await
        .ok()?
        .json::<Vec<RemoteRepo>>()
        .await
        .ok()?;
    if !repos.is_empty() {
        return Some(repos);
    }
    return None;
}

async fn list_github_team_repos(
    token: &String,
    github_team_repo_url: &Url,
    github_team_prefix: &String,
) -> Vec<RemoteRepo> {
    let client = Client::new();
    let mut repos: Vec<RemoteRepo> = Vec::new();
    let mut page = 1;
    while let Some(page_repos) = get_repos(&client, &token, page, github_team_repo_url).await {
        repos.extend(page_repos);
        page += 1;
    }
    return repos
        .into_iter()
        .filter(|repo| !repo.archived)
        .filter(|repo| repo.name.starts_with(github_team_prefix.as_str()))
        .collect::<Vec<RemoteRepo>>();
}

struct GitMessage {
    name: String,
    message: String,
}

#[derive(Debug, Clone)]
struct LocalRepo {
    name: String,
    path: PathBuf,
}

fn list_local_repos(path: &PathBuf) -> Vec<LocalRepo> {
    let mut repos: Vec<LocalRepo> = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if let Ok(subdir) = entry {
                if is_git_repo(&subdir.path().to_string_lossy().to_string()) {
                    repos.push(LocalRepo {
                        name: subdir.file_name().into_string().unwrap(),
                        path: subdir.path(),
                    });
                }
            }
        }
    }
    repos
}

fn is_known_repo(
    remote_repo: &RemoteRepo,
    local_repos: &Vec<LocalRepo>,
    github_team_prefix: &String,
) -> bool {
    for local_repo in local_repos {
        if local_repo.name == remote_repo.name.replace(github_team_prefix.as_str(), "") {
            return true;
        }
    }
    return false;
}

fn parse_command_line_arguments() -> clap::ArgMatches {
    clap::Command::new("reposync")
        .about("tool to keep team repos up to date.")
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
        let _ = match git_pull(local_repo.clone()) {
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
        let _ = git_clone(
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

fn print_git_message(messages: Vec<GitMessage>, title_prefix: String) {
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

fn find_new_repos(
    remote_repos: &Vec<RemoteRepo>,
    local_repos: &Vec<LocalRepo>,
    github_team_prefix: &String,
) -> Vec<RemoteRepo> {
    remote_repos
        .iter()
        .filter(|repo| !is_known_repo(repo, local_repos, github_team_prefix))
        .cloned()
        .collect()
}

#[tokio::main]
async fn main() {
    let matches = parse_command_line_arguments();

    let repo_root_dir = matches.get_one::<PathBuf>("repo_root_dir").unwrap();
    let token = matches.get_one::<String>("github_token").unwrap();
    let github_team_repo_url = matches.get_one::<Url>("github_team_repo_url").unwrap();
    let github_team_prefix = matches.get_one::<String>("github_team_prefix").unwrap();

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

    print_git_message(error_messages, "ERROR in".to_string());
    print_git_message(ok_messages, "Info for".to_string());
}
